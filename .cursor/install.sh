#!/usr/bin/env bash
# Idempotent setup for the trace-server backend and its Postgres database.
#
# Durable, source-derived work lives here so it can be baked into an
# environment snapshot: the Rust toolchain, the Postgres packages, the
# database + schema, and a warmed build. Per-boot service startup lives in
# start.sh instead, because a running process is not part of a snapshot.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_DIR="$REPO_ROOT/src/trace-server"
PG_VERSION=16

echo "==> Rust toolchain"
# The committed Cargo.lock pulls dependencies that require a newer Rust edition
# than some base images ship, so pin to current stable (matches the Dockerfile's
# `rust:1` image).
rustup update stable
rustup default stable
rustc --version

echo "==> PostgreSQL ${PG_VERSION}"
if ! command -v pg_ctlcluster >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
    postgresql postgresql-contrib
fi

echo "==> Start cluster (needed to create the role/db and migrate)"
sudo pg_ctlcluster "${PG_VERSION}" main start 2>/dev/null || true
for _ in $(seq 1 30); do
  sudo -u postgres pg_isready -q && break
  sleep 1
done

echo "==> Role and database"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='trace'" | grep -q 1 \
  || sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE ROLE trace LOGIN PASSWORD 'trace';"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='trace'" | grep -q 1 \
  || sudo -u postgres createdb -O trace trace

echo "==> Apply migrations"
bash "$REPO_ROOT/.cursor/apply-migrations.sh"

echo "==> Warm the build"
cd "$SERVER_DIR"
cargo build --locked

echo "==> install.sh done"
