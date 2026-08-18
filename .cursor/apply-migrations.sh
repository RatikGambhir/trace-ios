#!/usr/bin/env bash
# Apply db/migrations/*.sql exactly once each, in filename order.
#
# The migrations are not all individually re-runnable (some use bare
# `CREATE TABLE`), so this tracks what has already been applied in a
# `schema_migrations` table and skips those. That makes both the install and
# the per-boot start path safe to run repeatedly against the same database.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PGPASSWORD=trace
PSQL=(psql -h localhost -U trace -d trace -v ON_ERROR_STOP=1 -q)

"${PSQL[@]}" -c "CREATE TABLE IF NOT EXISTS schema_migrations (
  filename   text PRIMARY KEY,
  applied_at timestamptz NOT NULL DEFAULT now()
);"

for f in "$REPO_ROOT"/db/migrations/*.sql; do
  name="$(basename "$f")"
  applied="$("${PSQL[@]}" -tAc "SELECT 1 FROM schema_migrations WHERE filename='${name}'")"
  if [ "$applied" = "1" ]; then
    echo "    - ${name} (already applied)"
    continue
  fi
  echo "    - ${name} (applying)"
  "${PSQL[@]}" -f "$f"
  "${PSQL[@]}" -c "INSERT INTO schema_migrations (filename) VALUES ('${name}');"
done
