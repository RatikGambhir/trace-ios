# Build stage
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

# Runtime stage — no toolchain, no build deps.
FROM debian:bookworm-slim AS runtime

# rustls verifies the Postgres certificate against the system roots.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app

COPY --from=builder /app/target/release/trace-server /usr/local/bin/trace-server

USER app
ENV PORT=8080
EXPOSE 8080

CMD ["trace-server"]
