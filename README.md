# trace-ios

Trace: a SwiftUI iOS app and its Axum backend.

## Layout

- `src/mobile-app/` — the SwiftUI iOS app (Xcode project).
- `src/trace-server/` — the Axum HTTP backend.
- `plan/` — planning and design docs.

## Mobile app

Open `src/mobile-app/Trace.xcodeproj` in Xcode 16 or later, pick an iOS
simulator, and run. Deployment target is iOS 17.0.

## Server

```sh
cd src/trace-server
cargo run
```

Listens on `127.0.0.1:3000`. See `src/trace-server/README.md` for endpoints and
configuration.

The two are not wired together yet — the app does not call the server.
