# trace-api

Axum service backing the Trace app. It owns the `users` table in the Railway
`trace` Postgres database.

## Endpoints

| Method | Path                 | Purpose                                   |
| ------ | -------------------- | ----------------------------------------- |
| GET    | `/health`            | Liveness — always 200 while the process is up |
| GET    | `/ready`             | Readiness — 200 only when Postgres answers    |
| POST   | `/api/v1/users`      | Insert a user                             |
| GET    | `/api/v1/users/{id}` | Read a user back (no secret columns)      |

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
| `RUST_LOG`     | no       | Defaults to `trace_api=info,tower_http=info`    |

## Running locally

```sh
export DATABASE_URL='postgres://user:pass@host:5432/railway'
cargo run
```

The schema is not created at startup — apply `db/migrations/0001_create_users.sql`
first.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

## Deployment

Deployed on Railway from this repo with the service root set to `/server`,
built from the `Dockerfile` here.

## Notes

CORS is currently `CorsLayer::permissive()`, and `POST /api/v1/users` is
unauthenticated — anyone who can reach the public URL can create a user. Both
need tightening before this holds real data.
