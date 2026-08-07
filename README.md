# trace-ios

Trace: a SwiftUI iOS app and its Axum backend.

## Layout

- `src/mobile-app/` — the SwiftUI iOS app (Xcode project).
- `src/trace-server/` — the Axum HTTP backend.
- `db/migrations/` — SQL migrations for the Postgres database.
- `plan/` — planning and design docs.

## Mobile app

Open `src/mobile-app/Trace.xcodeproj` in Xcode 16 or later, pick an iOS
simulator, and run. Deployment target is iOS 17.0.

## Server

```sh
cd src/trace-server
export DATABASE_URL='postgres://user:pass@host:5432/railway'
cargo run
```

Listens on `0.0.0.0:8080` by default (`PORT` overrides it). A reachable Postgres
is required at startup. See `src/trace-server/README.md` for endpoints and
configuration, and `db/migrations/` for the schema.

Deployed on Railway at <https://api-production-946d.up.railway.app>.

The two are not wired together yet — the app does not call the server.
