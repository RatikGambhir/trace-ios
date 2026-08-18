#!/usr/bin/env bash
# Per-boot reconciliation: bring Postgres back up and make sure the schema is
# present, then return. A snapshot preserves the database files on disk but not
# the running server process, so every boot has to restart the cluster.
#
# This is idempotent and cheap: it starts the cluster only if it isn't already
# accepting connections, and creating the role/db is guarded. Migrations are
# written to be safe to re-run, so re-applying them reconciles a pod that booted
# from a base image without the schema.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PG_VERSION=16

echo "==> Start PostgreSQL ${PG_VERSION}"
sudo pg_ctlcluster "${PG_VERSION}" main start 2>/dev/null || true
for _ in $(seq 1 30); do
  sudo -u postgres pg_isready -q && break
  sleep 1
done
sudo -u postgres pg_isready

echo "==> Ensure role, database, and schema"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='trace'" | grep -q 1 \
  || sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE ROLE trace LOGIN PASSWORD 'trace';"
sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='trace'" | grep -q 1 \
  || sudo -u postgres createdb -O trace trace

bash "$REPO_ROOT/.cursor/apply-migrations.sh"

echo "==> start.sh done — Postgres ready on localhost:5432 (db 'trace')"
