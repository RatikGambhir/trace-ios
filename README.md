# trace-ios

A basic SwiftUI iOS app.

## Layout

- `src/Trace/` — the Xcode project and app source.
- `server/` — the Axum backend API. See [server/README.md](server/README.md).
- `db/migrations/` — SQL migrations for the Postgres database.
- `plan/` — planning and design docs.

## Running it

Open `src/Trace/Trace.xcodeproj` in Xcode 16 or later, pick an iOS simulator,
and run. Deployment target is iOS 17.0.
