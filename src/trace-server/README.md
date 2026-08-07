# trace-server

A basic [Axum](https://github.com/tokio-rs/axum) backend for Trace.

## Running

```sh
cargo run
```

Listens on `127.0.0.1:3000` by default. Override with `TRACE_SERVER_ADDR`:

```sh
TRACE_SERVER_ADDR=0.0.0.0:8080 cargo run
```

Log level comes from `RUST_LOG` and defaults to
`trace_server=debug,tower_http=debug`.

## Endpoints

| Method | Path      | Response                              |
| ------ | --------- | ------------------------------------- |
| GET    | `/`       | `trace-server` (plain text)           |
| GET    | `/health` | `{"status":"ok","version":"0.1.0"}`   |
| POST   | `/echo`   | Echoes back the posted `message`      |

```sh
curl localhost:3000/health
curl -X POST localhost:3000/echo \
  -H 'content-type: application/json' \
  -d '{"message":"hello"}'
```

A malformed `/echo` body returns 422; unknown paths return 404.

## Tests

```sh
cargo test
```

Tests drive the router in-process via `tower::ServiceExt::oneshot`, so no port
is bound.
