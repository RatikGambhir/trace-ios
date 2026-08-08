# trace-server

Axum service backing the Trace app. It owns the journeys schema — `users`,
`journeys`, `journey_legs`, `journey_flights`, `journey_drives`, `places`,
`vehicles`, `airports`, `airlines`, and `flights` — in the Railway `trace`
Postgres database.

## Endpoints

| Method | Path                 | Purpose                                   |
| ------ | -------------------- | ----------------------------------------- |
| GET    | `/health`            | Liveness — always 200 while the process is up |
| GET    | `/ready`             | Readiness — 200 only when Postgres answers    |
| POST   | `/api/v1/users`      | Insert a user                             |
| GET    | `/api/v1/users/{id}` | Read a user back (no secret columns)      |
| POST   | `/api/v1/journeys`   | Record a trip and everything it is made of |
| GET    | `/api/v1/journeys/{id}` | Read a trip back with its segments |

### `POST /api/v1/users`

```json
{
  "first_name": "Ada",
  "last_name": "Lovelace",
  "role": "admin",
  "password": "correct horse battery staple"
}
```

`role` is optional and defaults to `user`. Responds `201` with the created row
plus the generated API key:

```json
{
  "id": "0f8f...",
  "first_name": "Ada",
  "last_name": "Lovelace",
  "role": "admin",
  "created_at": "2026-08-06T18:40:00Z",
  "updated_at": "2026-08-06T18:40:00Z",
  "api_key": "trace_sk_..."
}
```

**The API key is returned exactly once.** No endpoint reads it back, so the
caller has to store it at creation time.

Errors come back as `{"error": "...", "details": [...]}` — `422` for validation
failures, `409` on a unique-constraint collision, `500` for anything else
(database detail is logged, never returned).

### `POST /api/v1/journeys`

The write path for the whole schema. One request records a trip, its ordered
segments, the places at either end of each one, and — per mode — the airport,
airline, and flight behind a flight segment, or the vehicle behind a drive.

```json
{
  "user_id": "8a5edb64-...",
  "idempotency_key": "england-aug-2026",
  "title": "England, August 2026",
  "started_at": "2026-08-10T18:00:00Z",
  "ended_at": "2026-08-24T12:00:00Z",
  "metadata": { "tags": ["holiday"] },
  "segments": [
    {
      "mode": "drive",
      "origin": { "type": "custom", "name": "Home", "city": "Brooklyn",
                  "latitude": 40.678178, "longitude": -73.944160 },
      "destination": { "type": "airport", "iata_code": "JFK",
                       "name": "John F. Kennedy International",
                       "latitude": 40.639751, "longitude": -73.778925 },
      "started_at": "2026-08-10T18:00:00Z",
      "ended_at": "2026-08-10T19:10:00Z",
      "distance_miles": 18,
      "drive": {
        "vehicle": { "type": "new", "nickname": "The Subaru", "make": "Subaru" },
        "role": "driver"
      }
    },
    {
      "mode": "flight",
      "flight": {
        "airline": { "iata_code": "AA", "name": "American Airlines" },
        "flight_number": "100",
        "origin": { "iata_code": "JFK" },
        "destination": { "iata_code": "LHR", "name": "London Heathrow",
                         "latitude": 51.47002, "longitude": -0.454295 },
        "scheduled_departure_at": "2026-08-10T22:00:00Z",
        "scheduled_arrival_at": "2026-08-11T10:00:00Z"
      },
      "booking": { "seat": "32A", "cabin": "economy", "booking_reference": "QK7P2M" }
    },
    { "mode": "walk", "duration_minutes": 12, "distance_miles": 1 }
  ]
}
```

Responds `201` with the trip, its `journey_totals` roll-up, and every segment
with its endpoints and per-mode details resolved. `GET /api/v1/journeys/{id}`
returns exactly the same shape.

#### Segments

`position` is the array index, so the order sent is the order stored — there is
no second field to keep in step.

Each mode admits exactly one detail object, and rejects the others:

| Mode | Detail | Notes |
| ---- | ------ | ----- |
| `flight` | `flight` (required) + `booking` | `origin`/`destination` must be **omitted** — they come from the flight |
| `drive` | `drive` (optional) | Vehicle, role, route |
| everything else | none | `train`, `bus`, `ferry`, `walk`, `bike`, `other` |

Endpoints take one of three forms:

- `{"type": "saved", "id": 42}` — a place already stored
- `{"type": "airport", "iata_code": "JFK", ...}` — resolved or created, then
  read back through the place the `airports_sync_place` trigger mirrors
- `{"type": "custom", "name": "Home", "latitude": …, "longitude": …}` — a new place

#### What the server derives

- **A flight segment's endpoints, distance, and duration** come from the flight
  it references. `distance_miles` is the great-circle distance between the
  airports' stored coordinates; duration is the scheduled block time.
- **Any segment's `duration_minutes`**, when both timestamps are given.
- A drive's `distance_miles` is *not* derived — road distance is not
  great-circle distance, so it is taken from the caller or left `null`.

#### Atomic, idempotent

**Atomic.** Journey, segments, places, airports, airlines, flights, and vehicles
all go in one transaction. A failure on the last segment leaves none of the
reference rows the earlier ones created.

**Idempotent**, opt-in via `idempotency_key` (unique per user). A flight has a
natural key; a journey does not — two trips can share a title and dates — so the
key is the caller's to choose. The journey row is written *first*, so a replay
conflicts before any downstream row is touched: it costs one statement and
creates nothing. Ten identical concurrent requests produce one `201`, nine
`200`s, and one trip. Without a key, every request creates a new journey.

Within one request, a waypoint described twice — the airport one leg ends at and
the next begins from — resolves to a single place. Across requests it does not:
use `{"type": "saved", "id": N}` for a place you already have.

| Status | When |
| ------ | ---- |
| `201` | The journey was created |
| `200` | An identical `idempotency_key` already existed; the stored trip is returned |
| `422` | Validation failed — every problem across every segment at once, each naming its segment (`segments[2].flight.origin.iata_code …`). Also an unknown `user_id`, place, or vehicle, and a new airport or airline given without a `name` |
| `409` | An `icao_code` already belongs to a different airport or airline, or a concurrent request is mid-insert (retry) |
| `503` | The request exceeded the 15s timeout |

## Schema

Migrations live in `db/migrations/` and are applied manually — the server does
not run them at startup.

| Migration | Contents |
| --------- | -------- |
| `0001_create_users.sql` | `users`, plus the shared `set_updated_at()` trigger function |
| `0002_create_flights_schema.sql` | `airports`, `airlines`, `flights` |
| `0003_add_updated_at_triggers.sql` | `updated_at` triggers on the three tables from 0002 |
| `0004_create_journeys_schema.sql` | `places`, `vehicles`, `journeys`, `journey_legs`, the per-mode leg tables, and the `journey_totals` view |
| `0005_add_journey_idempotency.sql` | `journeys.idempotency_key`, and the unique indexes behind journey and vehicle get-or-create |
| `0006_rename_segment_tables.sql` | Renames `journey_segments`→`journey_legs`, `segment_flights`→`journey_flights`, `segment_drives`→`journey_drives`, and their constraints, indexes, and triggers |

### Journeys

A journey is one user's trip, made of ordered segments. Everything every mode
of transport has — when, where, how long, how far — lives on `journey_legs`,
so a trip total is one `SUM` across flights, drives, and anything added later:

```sql
SELECT * FROM journey_totals WHERE journey_id = $1;
```

Each mode that needs more columns gets its own small table keyed on the segment:

- `journey_flights` → references `flights(id)`, plus seat, cabin, and booking
  reference. A flight is shared — one `flights` row serves every user aboard —
  so this table holds only what is personal to the traveller. Flight number and
  schedule stay on `flights`.
- `journey_drives` → references `vehicles(id)`, plus role and route. A drive has
  no shared counterpart, so its details live here outright.
- Modes with nothing extra to say (`walk`, `bike`, `bus`, …) need no subtype row
  at all.

**The database says *leg*, the API says *segment*.** The tables were renamed to
a `journey_` prefix without touching the wire contract, so requests and responses
still carry a `segments` array and errors still read `segments[2].flight…`.
Renaming the API too is a mechanical follow-up if you want one vocabulary
throughout; it would be a breaking change to the contract, which nothing consumes
yet.

Subtype tables foreign-key `(leg_id, mode)` against
`journey_legs (id, mode)`, so a drive cannot attach to a flight leg, and
a segment's mode cannot be changed while its details exist. Adding a mode later
is one new table plus one value in the `mode` CHECK.

`places` is the endpoint of any segment — an airport, an address, a landmark —
and is what the globe view reads markers from. Airports are mirrored into it
automatically by the `airports_sync_place` trigger on insert and update, so
`POST /api/v1/flights` populates `places` as a side effect and no application
code has to remember to.

## Passwords and keys

Passwords are hashed with Argon2id and a per-user random salt, on a blocking
thread so the async executor isn't stalled. The plaintext is never logged, never
stored, and never serialised — `ValidatedUser` deliberately has no `Debug` impl.
API keys are 256 bits from the OS CSPRNG, prefixed `trace_sk_`.

## Configuration

| Variable       | Required | Notes                                          |
| -------------- | -------- | ---------------------------------------------- |
| `DATABASE_URL` | yes      | On Railway, set to `${{Postgres.DATABASE_URL}}` |
| `PORT`         | no       | Defaults to `8080`; Railway injects it          |
| `RUST_LOG`     | no       | Defaults to `trace_server=info,tower_http=info` |

## Layout

```text
src/
├── main.rs            configuration, wiring, serving
├── router.rs          route table and middleware stack
├── handlers/          HTTP in, status code out — no logic of their own
├── services/          transactions, idempotency, derived values
├── repositories/      SQL, on a connection the caller owns
├── models/
│   ├── requests/      what arrives, plus the validation that checks it
│   ├── entities/      what the database stores
│   └── responses/     what goes back
└── core/
    ├── sql_builder/   the SQL builder the repositories query through
    └── …              error shape, geometry, crypto, validation helpers, state
```

Request and response types are named for the operation they belong to —
`InsertUserRequest`/`InsertUserResponse`, `InsertJourneyRequest`/
`InsertJourneyResponse`, `GetJourneyResponse` — so a signature says which
endpoint it serves. Each request validates into a `Validated*` twin beside it;
nothing downstream of `models::requests` ever sees a raw body.

Each layer only calls the one below it. Repositories take a `&mut PgConnection`
rather than a pool, which is what lets a journey write nine tables atomically:
the service opens one transaction and hands the same connection to each
repository in turn.

### The SQL builder

Repositories write their statements through `core::sql_builder` rather than as
string literals with `$1`…`$16` in them. The problem with the literal form is
narrow but real: to answer "what goes into `arrival_gate`?" you count
placeholders in the SQL, then count `.bind()` calls underneath, and trust that
the two lists agree. Here they are one list:

```rust
let inserted: Option<i64> = Insert::into("airlines")
    .set("iata_code", &airline.iata_code)
    .set("icao_code", &airline.icao_code)
    .set("name", name)
    .on_conflict_do_nothing(&["iata_code"])
    .returning(&["id"])
    .fetch_optional_scalar(conn)
    .await?;
```

A `Select` reads the same way, and renders its clauses in SQL's order whatever
order they were called in:

```rust
let places: Vec<PlaceResponse> = Select::from("places p")
    .columns(&["p.id", "p.name", "a.iata_code"])
    .left_join("airports a", "a.id = p.airport_id")
    .where_any_of("p.id", place_ids)
    .fetch_all(conn)
    .await?;
```

Two rules hold it together:

- **Everything that becomes SQL text is `&'static str`** — tables, columns, join
  predicates, casts. All of them are literals in this repository's source, which
  the compiler checks; a `String` built from a request body will not typecheck.
  There is no escaping to get wrong because there is nothing to escape.
- **Everything that came from outside is a bind parameter.** `Params::bind` is
  the only thing that writes a `$n`, and it writes one only as it appends the
  value that placeholder names. A placeholder without its argument is not
  expressible.

`RETURNING` takes the same column list a `SELECT` of that row would, so an
insert and its read-back share one constant instead of drifting apart —
`repositories/flights.rs` uses that for all nineteen columns of a flight.

Scope is `SELECT` and `INSERT`, because that is what a journey read and write
need. No `UPDATE`, no `DELETE`, no subqueries, no `GROUP BY`: a query that wants
one of those is written out in SQL and handed to sqlx directly. The builder is
worth having while it stays smaller than the SQL it replaces.

Every builder can `to_sql()` itself, which is how `core/sql_builder/*_tests.rs`
read — assert on the exact statement, no database needed.

Tests sit next to the code they cover, as `<file>_tests.rs` —
`models/requests/journey.rs` is tested by `models/requests/journey_tests.rs`,
wired in with:

```rust
#[cfg(test)]
#[path = "journey_tests.rs"]
mod tests;
```

They stay unit tests rather than moving to a top-level `tests/` directory, so
they keep access to private items and need no visibility widened just to be
tested.

## Running locally

```sh
export DATABASE_URL='postgres://user:pass@host:5432/railway'
cargo run
```

The schema is not created at startup — apply the migrations in `db/migrations/`
first.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

The router tests drive `app()` directly with `oneshot`, using a lazily-connected
pool, so `cargo test` needs no database.

## Deployment

Deployed on Railway (project `trace`, service `api`) from this repo with the
service root set to `/src/trace-server`, built from the `Dockerfile` here.

Base URL: <https://api-production-946d.up.railway.app>

`DATABASE_URL` is a `${{Postgres.DATABASE_URL}}` reference, so the API reaches
the database over Railway's private network — the database does not need to be
publicly exposed for the API to work.

## Notes

CORS is currently `CorsLayer::permissive()`, and both `POST` endpoints are
unauthenticated. `POST /api/v1/journeys` takes `user_id` from the request body,
so anyone who can reach the public URL can write a trip into any account. That
field should come from an authenticated principal, not the body — it is the
first thing to fix before this holds real data.

Two smaller gaps:

- There is no standalone flights endpoint any more. Flights are created as part
  of a journey segment, and read back through the journey. A flight nobody
  travelled on has no way in.
- Places are deduplicated within a request, not across them. Posting two
  journeys that both describe "Home" inline creates two rows; pass
  `{"type": "saved", "id": N}` to reuse one. Doing better needs a decision on
  whether `places` is a global table or scoped per user — `Home` means a
  different building for each of them.
