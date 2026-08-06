# Roadmap

## Where things stand

A minimal SwiftUI iOS app exists at `src/Trace`:

- `TraceApp.swift` — the `@main` entry point, one `WindowGroup`.
- `ContentView.swift` — a single screen with a tap counter.
- `Trace.xcodeproj` — iOS app target, deployment target iOS 17.0, bundle id
  `com.example.Trace`.

The project uses Xcode's file-system-synchronized groups, so new `.swift` files
dropped into `src/Trace/Trace/` are picked up by the target automatically — no
`project.pbxproj` edits needed.

## Open decisions

- **Bundle identifier** — currently the placeholder `com.example.Trace`. Needs a
  real reverse-DNS id before anything goes to a device or TestFlight.
- **What Trace actually does** — the app is a shell. The feature set drives
  everything below.
- **Persistence** — SwiftData vs. plain files vs. none, once there's data worth
  keeping.

## Next steps

1. Decide the app's core feature and replace `ContentView` with the real first
   screen.
2. Add a unit test target and a smoke test, so there's somewhere for tests to go
   from the start.
3. Set the real bundle identifier and signing team.
4. Add CI (build + test on a macOS runner) once there's a test to run.
