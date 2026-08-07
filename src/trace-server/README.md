# trace-server

Axum service backing the Trace app. It owns the `users`, `airports`, `airlines`,
and `flights` tables in the Railway `trace` Postgres database.

## Endpoints

| Method | Path                 | Purpose                                   |
| ------ | -------------------- | ----------------------------------------- |
| GET    | `/health`            | Liveness — always 200 while the process is up |
| GET    | `/ready`             | Readiness — 200 only when Postgres answers    |
| POST   | `/api/v1/users`      | Insert a user                             |
| GET    | `/api/v1/users/{id}` | Read a user back (no secret columns)      |
| POST   | `/api/v1/flights`    | Record a flight, creating its airports and airline as needed |

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

### `POST /api/v1/flights`

Records one flight. The airline and both airports are identified by IATA code
and created on the way through if the database has not seen them yet, so the
caller never has to pre-register reference data.

```json
{
  "airline": { "iata_code": "AA", "icao_code": "AAL", "name": "American Airlines" },
  "flight_number": "100",
  "origin": {
    "iata_code": "JFK",
    "icao_code": "KJFK",
    "name": "John F. Kennedy International",
    "city": "New York",
    "country_code": "US",
    "latitude": 40.639751,
    "longitude": -73.778925,
    "timezone": "America/New_York"
  },
  "destination": { "iata_code": "LHR", "name": "London Heathrow", "latitude": 51.47002, "longitude": -0.454295 },
  "scheduled_departure_at": "2026-08-10T22:00:00Z",
  "scheduled_arrival_at": "2026-08-11T10:00:00Z",
  "departure_terminal": "8",
  "departure_gate": "A1",
  "aircraft_type": "Boeing 777-300ER",
  "aircraft_registration": "N58031"
}
```

Optional throughout: `airline.icao_code`, every airport field except
`iata_code`, `status` (defaults to `scheduled`), `actual_departure_at`,
`actual_arrival_at`, the terminal and gate fields, and the two aircraft fields.
Codes are case-insensitive and stored upper-case.

Responds `201` with the created row:

```json
{
  "id": "2934...",
  "airline_id": 1,
  "flight_number": "100",
  "origin_airport_id": 1,
  "destination_airport_id": 2,
  "distance_miles": 3443,
  "scheduled_departure_at": "2026-08-10T22:00:00Z",
  "scheduled_arrival_at": "2026-08-11T10:00:00Z",
  "status": "scheduled",
  "...": "the remaining flights columns"
}
```

**Distance is derived, not accepted.** `distance_miles` is the great-circle
distance between the two airports' stored coordinates, rounded to whole miles.
Send coordinates for any airport the database does not have yet; if either
airport ends up with no coordinates on file, `distance_miles` is `null` rather
than a guess.

**Atomic.** Airports, airline, and flight are written in a single transaction.
If any step fails, nothing is left behind — not even an airport that was
inserted moments earlier in the same request.

**Idempotent.** `(airline, flight_number, scheduled_departure_at)` is the
flight's natural key, so replaying a request is safe: the second call returns
`200` with the row the first call created, unchanged. Airports and airlines are
get-or-create and never overwrite what is already stored, so a request naming an
airport that already exists may send just its `iata_code`. Twelve identical
concurrent requests produce one `201`, eleven `200`s, and one row.

| Status | When |
| ------ | ---- |
| `201` | The flight was created |
| `200` | An identical flight already existed; the stored row is returned |
| `422` | Validation failed — every problem is listed at once. Also covers a new airport or airline given without a `name`, which is required to insert it |
| `409` | An `icao_code` already belongs to a different airport or airline, or a concurrent request is mid-insert (retry) |

## Schema

Migrations live in `db/migrations/` and are applied manually — the server does
not run them at startup.

| Migration | Contents |
| --------- | -------- |
| `0001_create_users.sql` | `users`, plus the shared `set_updated_at()` trigger function |
| `0002_create_flights_schema.sql` | `airports`, `airlines`, `flights` |
| `0003_add_updated_at_triggers.sql` | `updated_at` triggers on the three tables from 0002 |

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
unauthenticated — anyone who can reach the public URL can create a user, or a
flight and the reference rows behind it. Both need tightening before this holds
real data.

There is no `GET /api/v1/flights/{id}` yet, so a created flight can only be read
back through the response to the `POST` that created it (or a replay of it).
