# Railway Bucket

Object storage for user-generated files — profile avatars, cover images, place
and journey photos — held in the Railway bucket `trace-bucket`, written and read
by the iOS app through short-lived presigned URLs that `trace-server` mints.

The app already has the holes this fills: `UserProfile.avatarURL`,
`UserProfile.coverImageURL`, `Place.imageURL`, and `PlaceCollection.coverImageURL`
are all `URL?` fed by mock data today. Nothing produces those URLs. This is the
plan for the thing that does.

## What "done" looks like

1. A user picks a photo in the app, and it lands in the bucket without the
   bytes ever passing through `trace-server`.
2. The row that points at it (`users.avatar_media_id`, a journey photo, a place
   image) is only written once the object is confirmed present — no database
   reference to an object that isn't there.
3. Reading a photo back is a stable, cacheable app-facing URL that redirects to
   a short-lived presigned GET. The bucket's credentials never leave the server,
   and no URL in the app is valid forever.
4. An upload that is abandoned half-way leaves a `pending` row that a sweep
   deletes, not a permanent orphan.
5. Production and any future PR environment write to different buckets with
   different credentials, with no configuration change beyond the variable
   references.

## The bucket as it stands

Confirmed against the Railway API for project `trace`
(`c27c08df-5b2e-4a25-bef1-185d81795869`, environment `production`):

- Services are `api` and `Postgres`. The bucket is a separate resource kind and
  does not appear in the service list.
- `api` currently has **no bucket variables** — `DATABASE_URL`, `RUST_LOG`, and
  the `RAILWAY_*` set, nothing else. Wiring the credentials is step one and
  nothing works before it.
- The bucket's `Files` tab reads *Empty directory*. Nothing has been written
  yet, so there is no legacy key layout to be compatible with. Pick the scheme
  deliberately now (see [Key layout](#key-layout)) — it is cheap today and
  expensive after a hundred thousand objects.

What Railway gives us, from the docs:

| Property | Value |
| --- | --- |
| Compatibility | Full S3 — put/get/head/delete, list v1+v2, copy, presigned URLs, tagging, multipart |
| Visibility | **Private only.** No public bucket URLs, no way to make one |
| URL style | Virtual-hosted (bucket as subdomain). The bucket's Credentials tab confirms per-bucket; older buckets are path-style |
| Endpoint | `https://storage.railway.app` |
| Networking | Public only — no private-network access, so the server's own calls go out over the internet |
| Per-environment | Each environment gets its own bucket instance and its own credentials |
| Not supported | Server-side encryption, versioning, object locks, lifecycle configuration |
| Billing | $0.015/GB-month. API operations and bucket egress free; **service** egress is not |
| Backing | Tigris. Encrypted at rest |

Two consequences worth internalising before designing anything:

- **"Private only" is the whole design constraint.** Every read is either a
  presigned URL or a proxy through the server. There is no third option, and
  there is no `https://trace-bucket.../avatar.jpg` to paste into `AsyncImage`.
- **No lifecycle configuration** means no bucket rule will ever clean up
  abandoned uploads for us. Whatever sweeps orphans has to be ours.

## Phase 0 — Auth, and why this is blocked on it

`POST /api/v1/journeys` takes `user_id` from the request body and both write
endpoints are unauthenticated. The server README already names this as the first
thing to fix. For journeys it is a data-integrity problem. For uploads it is
worse:

> An unauthenticated endpoint that mints presigned PUT URLs is an open file host
> with Railway's billing attached.

Anyone who can reach the public URL could mint upload grants in a loop, and the
only ceilings are the ones we sign into each URL. So: **API-key authentication
lands before the first media endpoint is routed**, not after.

The minimum that unblocks this work — not full auth, just enough:

- A `tower` middleware reading `Authorization: Bearer trace_sk_…`, hashing it
  the same way `core::crypto` hashes it at creation, and looking up the user.
- The resolved user id in a request extension, extracted by handlers as an
  authenticated principal.
- `401` when absent or unknown.
- Per-user rate limiting on the reserve endpoint (see
  [Abuse ceilings](#abuse-ceilings)).

Everything below assumes a handler can ask "who is calling?" and get an answer
it did not read out of the body.

## Shape of the integration

Three legs, and the middle one does not involve the server at all:

```text
  iOS app                    trace-server                  trace-bucket
     │                            │                             │
     │  1. POST /media/uploads    │                             │
     │    {content_type, bytes}   │                             │
     │ ─────────────────────────► │                             │
     │                            │  mint key, INSERT pending   │
     │                            │  presign PUT (signed        │
     │                            │  content-type + length)     │
     │  ◄──────────────────────── │                             │
     │    {media_id, url, headers, expires_at}                  │
     │                            │                             │
     │  2. PUT <presigned url>    │                             │
     │ ───────────────────────────┼───────────────────────────► │
     │  ◄─────────────────────────┼──────────────────────────── │
     │                            │                             │
     │  3. POST /media/{id}/commit│                             │
     │ ─────────────────────────► │  HEAD object ─────────────► │
     │                            │  ◄───── size, content-type  │
     │                            │  UPDATE status='committed'  │
     │  ◄──────────────────────── │                             │
     │    {media_id, url: /api/v1/media/{id}/content}           │
```

Reads are the same trick backwards: `GET /api/v1/media/{id}/content` checks
authorisation, presigns a GET, and answers `302` to it. The bytes travel
bucket → device.

### Why not just proxy the bytes

| Approach | Server egress | Memory | Auth control | Complexity |
| --- | --- | --- | --- | --- |
| **Presigned URLs both ways** | None | None | At mint time | Two-phase write, orphan sweep |
| Proxy uploads and reads through the server | Every byte, billed | Streaming buffers per request | Per request | One endpoint, no orphans |
| Presigned read, proxied write | Half | Upload buffers | Per request | Middle ground |

**Recommendation: presigned both ways.** Bucket egress is free and service
egress is not, so proxying means paying to move bytes that could have moved for
nothing. It also keeps a 12MP photo out of the server's memory entirely, which
matters more than it sounds on a small Railway instance handling several
concurrent uploads.

The cost of that choice is honest and worth stating: **the server does not see
the bytes**, so it cannot validate the file's contents at upload time. That is
what [commit](#the-commit-step) and the signed-header ceilings are for. If image
transformation (thumbnails, resizing) ever becomes a requirement, it arrives as
a background job reading from the bucket, not as a proxy on the upload path.

## Choosing the S3 client

The server needs to: presign PUT, presign GET, `HEAD` an object, `DELETE` an
object. That is a small surface, and the two realistic crates sit at opposite
ends of a real trade-off.

| Crate | What it does | Build cost | Fit |
| --- | --- | --- | --- |
| **`aws-sdk-s3`** + `aws-config` | Everything, SigV4 included, `presigned()` on any operation | Heavy — pulls `aws-smithy-*`, its own hyper stack; adds real minutes to the Docker build | Correct by default, nothing to get wrong |
| **`rusty-s3`** | Signing only. Builds signed URLs; you issue the request yourself | Tiny, no runtime deps | Needs an HTTP client (`reqwest`) for HEAD/DELETE |
| Hand-rolled SigV4 | — | None | No |

**Recommendation: `aws-sdk-s3`.** SigV4 presigning is a thing to get exactly
right, and a subtle mistake shows up as a `403` from Tigris with no useful
detail. This codebase's minimalism (a hand-written SQL builder rather than an
ORM) is about *not paying for what it doesn't use*, and here the alternative to
the SDK is not "less code" but "our own signing code" — which is the wrong side
of that trade.

Pin it, configure it explicitly rather than through `aws-config`'s environment
discovery, and keep it behind `core::bucket` so the choice is one module deep:

```rust
// core/bucket.rs — illustrative; confirm signatures against the pinned version.
let creds = Credentials::new(&config.access_key_id, &config.secret_access_key,
                             None, None, "railway");
let s3_config = aws_sdk_s3::config::Builder::new()
    .region(Region::new(config.region.clone()))
    .endpoint_url(&config.endpoint)
    .credentials_provider(creds)
    // Virtual-hosted is the default and what current Railway buckets use.
    // If the Credentials tab says path-style, set .force_path_style(true).
    .build();
```

If the Docker build time turns out to be intolerable, `rusty-s3` + `reqwest` is
a contained swap — same module, same public functions.

> **Note on presigned POST.** Railway's docs show `createPresignedPost` with a
> `content-length-range` condition. That is an AWS **JS** SDK feature; the Rust
> SDK presigns PUT/GET, not POST policies. The equivalent ceiling in a presigned
> PUT is to sign `Content-Length` and `Content-Type` into the request — S3
> rejects an upload whose headers don't match what was signed. Same protection,
> different mechanism, and the app must send exactly the headers it was handed.

## Key layout

The client never chooses a key. The server mints it, stores it, and hands back
an opaque `media_id`; the key is an implementation detail the app never sees.

```text
users/{user_id}/avatar/{uuid}.{ext}
users/{user_id}/cover/{uuid}.{ext}
journeys/{journey_id}/photos/{uuid}.{ext}
places/{place_id}/photos/{uuid}.{ext}
```

Rules behind it:

- **Owner-scoped prefix, always.** A key derived from a client-supplied filename
  is a path-traversal and overwrite bug waiting to be found (`../`, or two users
  both uploading `avatar.jpg`).
- **A UUID, not the original name.** Uploads are immutable — replacing an avatar
  writes a new object and repoints the row, rather than overwriting a key some
  cached URL still refers to.
- **Extension from the validated content type**, not from the filename. It is
  cosmetic (helps when browsing the bucket) and never load-bearing.
- **No environment prefix.** Environments already have separate buckets; adding
  `prod/` on top is redundant noise.

## Data model

One migration, `db/migrations/0007_create_media_schema.sql`, in the same
transactional style as 0004–0006.

```sql
BEGIN;

CREATE TABLE media_objects (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Who may read it back, and whose prefix it lives under.
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    bucket_key      TEXT NOT NULL UNIQUE,
    content_type    VARCHAR(100) NOT NULL,
    byte_size       BIGINT,                  -- NULL until committed
    checksum        TEXT,                    -- ETag as returned by HEAD

    -- 'pending'   — reserved, upload may or may not have happened
    -- 'committed' — HEAD confirmed the object exists
    status          VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'committed')),

    -- What it is for. Drives the prefix and the size/type ceilings.
    purpose         VARCHAR(30) NOT NULL
        CHECK (purpose IN ('avatar', 'cover', 'journey_photo', 'place_photo')),

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    committed_at    TIMESTAMPTZ,

    CONSTRAINT media_committed_fields_check
        CHECK ((status = 'committed') = (committed_at IS NOT NULL AND byte_size IS NOT NULL))
);

-- The orphan sweep's only query.
CREATE INDEX idx_media_pending ON media_objects (created_at) WHERE status = 'pending';
CREATE INDEX idx_media_user ON media_objects (user_id);

ALTER TABLE users
    ADD COLUMN avatar_media_id UUID REFERENCES media_objects(id) ON DELETE SET NULL,
    ADD COLUMN cover_media_id  UUID REFERENCES media_objects(id) ON DELETE SET NULL;

CREATE TRIGGER media_objects_set_updated_at ...  -- if an updated_at column is added

COMMIT;
```

Notes on the shape:

- **`media_objects` is the single registry.** Journey and place photos get join
  tables (`journey_media`, `place_media`, each with a `position`) when those
  features land; the registry itself does not change to accommodate them. Only
  the `purpose` CHECK grows, exactly like `journey_legs.mode` does.
- **`ON DELETE SET NULL`, not `CASCADE`**, from `users` — deleting a media row
  should blank an avatar, never delete the user.
- **Deleting a `media_objects` row does not delete the object.** Postgres cannot
  reach into the bucket. Deletion is a two-step in the service layer: `DELETE`
  the object, then the row — in that order, so a failure leaves a row pointing
  at a missing object (recoverable, visible) rather than an object no row knows
  about (invisible, billed forever).

### The commit step

Commit is what makes a two-phase write worth the extra round trip. On
`POST /api/v1/media/{id}/commit` the server:

1. Loads the pending row and checks it belongs to the caller.
2. `HEAD`s the object. Absent → `422`, the upload never happened.
3. Compares the returned `Content-Type` and `Content-Length` against what was
   reserved. A mismatch shouldn't be possible (both were signed), so treat it as
   a `422` and log loudly.
4. `UPDATE`s status, `byte_size`, `checksum`, `committed_at`.

That `UPDATE` is the first in the codebase. Per the server README the SQL
builder is `SELECT`/`INSERT` only, so this one is written out as sqlx SQL and
handed over directly — *not* an excuse to grow the builder for a single caller.

### Orphan sweep

Pending rows older than 24 hours are abandoned uploads. Two options:

- **In-process `tokio` interval task** started in `main`, running hourly:
  `DELETE FROM media_objects WHERE status='pending' AND created_at < NOW() - INTERVAL '24 hours'`,
  deleting each object first. Simple, no new infrastructure; runs once per
  replica (harmless — the deletes are idempotent).
- A Railway cron service hitting an internal endpoint. More moving parts, and
  the endpoint needs its own protection.

**Start with the in-process task.** It is ~30 lines and can be lifted out later
if the server ever runs on more than a couple of replicas.

## API surface

All four require authentication. All operate on the caller's own media.

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/v1/media/uploads` | Reserve a key, return a presigned PUT |
| POST | `/api/v1/media/{id}/commit` | Confirm the object landed |
| GET | `/api/v1/media/{id}/content` | `302` to a presigned GET |
| DELETE | `/api/v1/media/{id}` | Delete object then row |

### `POST /api/v1/media/uploads`

```json
{ "purpose": "avatar", "content_type": "image/jpeg", "byte_size": 482913 }
```

`201`:

```json
{
  "media_id": "3f2a…",
  "upload": {
    "method": "PUT",
    "url": "https://trace-bucket-a1b2c3.storage.railway.app/users/…?X-Amz-…",
    "headers": { "Content-Type": "image/jpeg", "Content-Length": "482913" },
    "expires_at": "2026-08-08T18:15:00Z"
  }
}
```

`422` on a content type outside the purpose's allow-list, or a `byte_size` over
its ceiling. The headers block is not advisory — it is what was signed, and S3
rejects anything else.

### `GET /api/v1/media/{id}/content`

`302` with `Location` set to a presigned GET and
`Cache-Control: private, max-age=300` — comfortably inside the presign's
lifetime so a cached redirect can never outlive the URL it points at. `404` for
an unknown or `pending` id.

Keeping this endpoint (rather than returning raw presigned URLs in every
response body) is what makes photo URLs storable: `users.avatar_media_id`
renders to `/api/v1/media/{id}/content`, which is stable forever, while the
thing it redirects to lives for minutes.

### Ceilings per purpose

Enforced at reserve time and signed into the URL:

| Purpose | Types | Max bytes |
| --- | --- | --- |
| `avatar` | `image/jpeg`, `image/png`, `image/heic` | 5 MB |
| `cover` | same | 10 MB |
| `journey_photo` | same | 15 MB |
| `place_photo` | same | 15 MB |

## Configuration

Five new variables on the `api` service, each a reference to the bucket so
nothing is copied and no environment holds another's credentials:

| Variable | Value | Required |
| --- | --- | --- |
| `S3_BUCKET` | `${{trace-bucket.BUCKET}}` | yes |
| `S3_ENDPOINT` | `${{trace-bucket.ENDPOINT}}` | yes |
| `S3_REGION` | `${{trace-bucket.REGION}}` | yes |
| `S3_ACCESS_KEY_ID` | `${{trace-bucket.ACCESS_KEY_ID}}` | yes |
| `S3_SECRET_ACCESS_KEY` | `${{trace-bucket.SECRET_ACCESS_KEY}}` | yes |
| `MEDIA_PRESIGN_TTL_SECS` | defaults: 900 upload, 300 download | no |

Railway's Credentials tab can inject these automatically with an AWS-SDK preset
(which names them `AWS_ACCESS_KEY_ID` etc.). **Use the explicit `S3_*` names
instead**, and pass them to the SDK config by hand. The AWS-named variables are
picked up by ambient credential discovery, and ambient discovery is how a
service ends up talking to the wrong bucket after an unrelated change. One
explicit read in `main`, like `DATABASE_URL`.

`BUCKET` is *not* `trace-bucket` — it is the display name plus a uniqueness
hash. Never hardcode it.

Follow `DATABASE_URL`'s precedent and **fail fast at startup** if any are
missing, so a misconfigured deploy dies immediately instead of serving `500`s on
the first upload. For local development, point the same five at MinIO
(`docker run -p 9000:9000 minio/minio server /data`) with
`S3_ENDPOINT=http://localhost:9000` and path-style forced — that also exercises
the path-style branch, which is otherwise dead code.

## Server layout

```text
src/
├── core/
│   └── bucket.rs          BucketConfig::from_env, the S3 client, presign helpers
├── handlers/
│   └── media.rs           HTTP in, status code out
├── services/
│   └── media.rs           reserve / commit / read / delete, and the sweep
├── repositories/
│   └── media.rs           SQL for media_objects
└── models/
    ├── requests/media.rs  ReserveUploadRequest + its Validated twin
    ├── entities/media.rs  MediaObject
    └── responses/media.rs ReserveUploadResponse, MediaResponse
```

`AppState` grows one field:

```rust
pub struct AppState {
    pub db: PgPool,
    pub bucket: Bucket,   // client + config; cheap to clone, held in the Arc
}
```

Every existing construction of `AppState` has to grow with it — including
`router_tests.rs`, which builds one against a lazily-connected pool. The bucket
equivalent is a `Bucket` built from dummy credentials pointing at an unroutable
endpoint: presigning is pure computation, so URL-shape assertions run with no
network, exactly as `sql_builder`'s tests assert on SQL text with no database.

## Implementation phases

### Phase 1 — Wire the credentials, prove the connection

Add the five variables as references on `api`. Add `aws-sdk-s3` to
`Cargo.toml`. Write `core::bucket` with `BucketConfig::from_env`, the client
constructor, and nothing else. Extend `/ready` — or better, add a `bucket` field
to its JSON — with a cheap `HeadBucket`.

**Checkpoint:** `/ready` reports the bucket reachable in production, and the
Docker build time is known.

### Phase 2 — Auth (Phase 0's blocking work)

The API-key middleware, the authenticated-principal extractor, `401`s, and
tests. No media endpoint is routed before this merges.

**Checkpoint:** an existing endpoint refuses an unauthenticated request.

### Phase 3 — Schema and reserve

Migration `0007`, the repository, and `POST /api/v1/media/uploads`. Validation
of purpose/type/size in `models::requests::media`, per the codebase's rule that
nothing downstream sees a raw body.

**Checkpoint:** a `curl` of the reserve endpoint returns a URL that a second
`curl -X PUT --upload-file` accepts, and the object appears in the bucket's
Files tab — which is currently empty, so it will be unmistakable.

### Phase 4 — Commit, read, delete

The `HEAD`-and-`UPDATE` commit, the `302` read, the delete. Wire
`users.avatar_media_id` through `GET /api/v1/users/{id}` so a committed avatar
comes back as `/api/v1/media/{id}/content`.

**Checkpoint:** full round trip by hand — reserve, upload, commit, follow the
redirect, get the image back; delete, and the read `404`s.

### Phase 5 — The sweep

The hourly `tokio` task, plus a log line per deleted orphan.

**Checkpoint:** a reserved-but-never-uploaded row disappears after the window,
with nothing else touched.

### Phase 6 — iOS

`src/mobile-app/Trace/Services/MediaService.swift`, plus whatever `Profile`
needs to call it. The project uses file-system-synchronized groups, so dropping
the file in is enough.

```swift
// Illustrative — not compiled; no macOS toolchain here.
func upload(_ data: Data, purpose: MediaPurpose, contentType: String) async throws -> UUID {
    let reservation = try await api.reserveUpload(
        purpose: purpose, contentType: contentType, byteSize: data.count
    )

    var request = URLRequest(url: reservation.upload.url)
    request.httpMethod = reservation.upload.method
    // Send exactly the signed headers — S3 rejects anything else.
    for (name, value) in reservation.upload.headers {
        request.setValue(value, forHTTPHeaderField: name)
    }
    let (_, response) = try await URLSession.shared.upload(for: request, from: data)
    guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw MediaError.uploadFailed }

    try await api.commitUpload(reservation.mediaID)
    return reservation.mediaID
}
```

Four app-side details that will otherwise be discovered the hard way:

- **`URLSession` follows the `302` and re-sends headers.** For
  `/media/{id}/content` that means the `Authorization` header travels to
  `storage.railway.app`. It is harmless (an API key, not a bucket credential,
  and the presigned URL already carries its own auth) but it is a token going
  somewhere it isn't needed. If that is unacceptable, implement
  `urlSession(_:task:willPerformHTTPRedirection:newRequest:)` and strip it.
- **`AsyncImage` caches by request URL**, which is the stable
  `/media/{id}/content` — so `URLCache` works normally and the presign churn
  underneath is invisible. This is the payoff for the redirect indirection.
- **Downscale before upload.** A 12MP HEIC straight from the camera is ~4MB for
  an avatar rendered at 96pt. Resize client-side; it keeps uploads inside the
  ceilings and the bucket bill near zero.
- **Background uploads.** A large journey photo over cellular wants
  `URLSessionConfiguration.background`, which means an upload task from a *file*
  URL, not `Data`. Worth structuring `MediaService` for from the start rather
  than retrofitting.

## Security notes

- **The client never picks the key, the purpose ceiling, or the expiry.** All
  three come from the server, and the URL is signed over them.
- **Treat every object as untrusted.** The server never sees the bytes, so a
  `Content-Type: image/jpeg` upload can contain anything. Since files are only
  ever handed back to the app as images and never served as a web page from our
  origin, the practical risk is low — but do not add an HTML-serving path later
  without revisiting this.
- **Strip EXIF client-side.** This is a travel app; photos carry GPS
  coordinates and timestamps. A user sharing a journey should not be
  broadcasting their home address in the metadata of a picture of their car.
  Cheap to do at the downscale step, and much harder to retrofit once photos
  are shared.
- **Presigned URLs can live up to 90 days.** Use minutes. A leaked 15-minute URL
  is a non-event; a leaked 90-day one is a public file.
- **Authorise on the read path, not just the write path.** `media_objects.user_id`
  is the check, and it must run before the presign — a `302` with a valid
  presigned URL *is* the grant.
- **Bucket CORS is not needed.** It applies to browser origins; iOS `URLSession`
  does not enforce it. It becomes a prerequisite the day a web client uploads.

### Abuse ceilings

An authenticated user can still mint reservations in a loop. Per-user rate
limits on reserve (say 60/hour), and a cap on `pending` rows per user (say 20)
rejected with `429`, bound the damage. Both are cheap and belong in Phase 3 —
they are the reason the sweep can be a simple time-based delete rather than a
quota reconciler.

## Cost

At $0.015/GB-month with free operations and free bucket egress, 10,000 avatars
at 200KB each is 2GB — **$0.03/month**. Journey photo albums are what would
actually move the number: 1,000 users × 50 photos × 2MB is 100GB, or $1.50/month.
The billing risk here is not the price per gigabyte, it is unbounded uploads,
which is what the ceilings and rate limits above exist for.

Note also that Railway bills for a deleted bucket's storage until the two-day
permanent deletion, so emptying before deleting matters if a bucket is ever
recreated.

## Testing

- **`core::bucket`** — presigning is pure computation against fixed
  credentials, so assert on the URL's shape (host, path, `X-Amz-Expires`, the
  signed-header list) with no network, the way `sql_builder` tests assert on SQL
  text with no database.
- **`models::requests::media`** — the validation table: every purpose × every
  rejected content type × the size boundary.
- **Router tests** — the four routes exist, and each answers `401`
  unauthenticated. `AppState` gains a dummy `Bucket`; nothing dials out.
- **Not unit tested** — that Tigris accepts our signatures. That is Phase 1's
  manual checkpoint against the real bucket, and there is no substitute for it.

## Risks and open questions

- **URL style.** The docs say current buckets are virtual-hosted and older ones
  path-style, and the Credentials tab is authoritative. `trace-bucket` is new,
  so virtual-hosted is near-certain — confirm in Phase 1 rather than debugging a
  `403` in Phase 3. Keep the toggle configurable either way; MinIO locally needs
  path-style regardless.
- **Build weight.** `aws-sdk-s3` may add meaningful time to a Docker build that
  currently compiles the whole tree on every deploy with no dependency-layer
  caching. If it hurts, either split the `Cargo.toml` copy into its own cached
  layer (with `cargo-chef`) or swap to `rusty-s3` — the first is worth doing
  independently of this work.
- **Buckets are public-network only.** Unlike `DATABASE_URL`'s private-network
  reference, the server's HEAD and DELETE calls leave Railway's network. Fine,
  but it means bucket reachability is an internet dependency of `/ready`.
- **No versioning, no lifecycle, no backups.** A deleted object is gone, and
  Railway offers no bucket snapshots. For avatars, acceptable. Before anything
  irreplaceable lives here (a user's only copy of a trip photo), decide whether
  that is still true.
- **`place_photo` ownership.** `places` is currently global and deduplicated
  only within a request — the server README flags the "is `places` global or
  per-user?" question as unresolved. A photo attached to a shared place inherits
  that ambiguity: whose photo is on `JFK`? Ship `avatar` and `cover` first,
  where ownership is unambiguous, and let the places decision arrive on its own
  schedule.
- **Multiple replicas.** The in-process sweep runs per replica. Idempotent, so
  harmless — but if the service ever scales past a couple of instances, move it
  to a cron service.
- **Thumbnails.** Not in scope. When they arrive, they are a background job
  reading and writing the bucket, plus a `variant` column — not a change to the
  upload path.

## Task checklist

- [ ] Confirm the Credentials tab's URL style for `trace-bucket`
- [ ] Add the five `S3_*` variable references to the `api` service
- [ ] `aws-sdk-s3` in `Cargo.toml`, pinned
- [ ] `core::bucket` — config, client, presign helpers
- [ ] `/ready` reports bucket reachability (Phase 1 checkpoint)
- [ ] API-key auth middleware + authenticated-principal extractor + `401`s
- [ ] Per-user rate limit and pending-row cap on reserve
- [ ] `db/migrations/0007_create_media_schema.sql`
- [ ] `repositories::media`, `services::media`, `handlers::media`
- [ ] `POST /api/v1/media/uploads` + validation tests
- [ ] `POST /api/v1/media/{id}/commit` (HEAD, then raw-sqlx `UPDATE`)
- [ ] `GET /api/v1/media/{id}/content` → `302`, with `Cache-Control`
- [ ] `DELETE /api/v1/media/{id}` — object first, row second
- [ ] `users.avatar_media_id` surfaced through `GET /api/v1/users/{id}`
- [ ] Hourly orphan sweep
- [ ] `MediaService.swift` — reserve, upload, commit
- [ ] Client-side downscale **and EXIF strip** before upload
- [ ] `ProfileView` avatar upload wired to real storage
- [ ] Dummy `Bucket` in `router_tests.rs`; presign-shape unit tests
- [ ] MinIO instructions in `src/trace-server/README.md`
- [ ] README: the new endpoints, the config table, the two-phase upload

## References

- [Storage Buckets | Railway](https://docs.railway.com/storage-buckets)
- [Uploading & Serving Files | Railway](https://docs.railway.com/storage-buckets/uploading-serving)
- [Storage Buckets Billing | Railway](https://docs.railway.com/storage-buckets/billing)
- [Use Storage Buckets for Uploads, Exports, and Assets | Railway](https://docs.railway.com/guides/storage-buckets-guide)
- [Variable references | Railway](https://docs.railway.com/variables)
- [Virtual hosted-style requests | Amazon S3](https://docs.aws.amazon.com/AmazonS3/latest/userguide/VirtualHosting.html)
- [`aws-sdk-s3` — presigned requests](https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/presigning/index.html)
