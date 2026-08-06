# Roadmap

## Where things stand

Two pieces, neither talking to the other yet.

### `src/mobile-app` — SwiftUI iOS app

- `TraceApp.swift` — the `@main` entry point, one `WindowGroup`.
- `ContentView.swift` — a single screen with a tap counter.
- `Trace.xcodeproj` — iOS app target, deployment target iOS 17.0, bundle id
  `com.example.Trace`.

The project uses Xcode's file-system-synchronized groups, so new `.swift` files
dropped into `src/mobile-app/Trace/` are picked up by the target automatically —
no `project.pbxproj` edits needed.

### `src/trace-server` — Axum backend

- `GET /` — plain-text name.
- `GET /health` — `{"status":"ok","version":"..."}`.
- `POST /echo` — echoes a JSON `message` back.

Request logging via `tower-http`'s `TraceLayer`, graceful shutdown on Ctrl-C,
bind address from `TRACE_SERVER_ADDR`. Two in-process router tests cover
`/health` and `/echo`.

## Open decisions

- **Bundle identifier** — currently the placeholder `com.example.Trace`. Needs a
  real reverse-DNS id before anything goes to a device or TestFlight.
- **What Trace actually does** — both halves are shells. The feature set drives
  everything below.
- **The API contract** — `/echo` is a placeholder. Real endpoints follow from
  the feature set, and the app's model types should mirror them.
- **Persistence** — the server has no storage. SwiftData vs. server-owned state
  vs. both, once there's data worth keeping.
- **Auth** — none yet, and it shapes both sides.

## Next steps

1. Decide the app's core feature and replace `ContentView` with the real first
   screen.
2. Define the first real endpoint, then add a small API client on the app side
   pointing at it.
3. Add a unit test target to the Xcode project and a smoke test, so there's
   somewhere for app-side tests to go from the start.
4. Set the real bundle identifier and signing team.
5. Add CI: `cargo test` for the server, `xcodebuild test` on a macOS runner for
   the app.
