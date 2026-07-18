#!/usr/bin/env bash
set -euo pipefail

# Applies the composed server migrator against PostgreSQL and validates the
# resulting schema. By default the ignored Rust integration test creates a
# fresh temporary database. RUSTOK_MIGRATION_SMOKE_REUSE_DB=1 reuses an
# existing database, which supports base-to-head upgrade verification.
#
# Configuration:
#   RUSTOK_MIGRATION_SMOKE_ADMIN_URL  Admin database URL used for CREATE/DROP DATABASE.
#                                     Defaults to postgres://postgres:postgres@localhost:5432/postgres.
#   RUSTOK_MIGRATION_SMOKE_DB_NAME    Optional temporary or existing database name.
#   RUSTOK_MIGRATION_SMOKE_KEEP_DB    Set to 1 to skip cleanup after the run.
#   RUSTOK_MIGRATION_SMOKE_INCREMENTAL
#                                     Set to 1 to apply migrations one-by-one.
#   RUSTOK_MIGRATION_SMOKE_REUSE_DB   Set to 1 to require and reuse an existing database.

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
export RUSTOK_MIGRATION_SMOKE_ADMIN_URL=${RUSTOK_MIGRATION_SMOKE_ADMIN_URL:-postgres://postgres:postgres@localhost:5432/postgres}
export RUSTOK_MIGRATION_SMOKE_DB_NAME=${RUSTOK_MIGRATION_SMOKE_DB_NAME:-rustok_migration_smoke_$(date +%Y%m%d%H%M%S)_$$}
export RUSTOK_MIGRATION_SMOKE_KEEP_DB=${RUSTOK_MIGRATION_SMOKE_KEEP_DB:-0}
export RUSTOK_MIGRATION_SMOKE_INCREMENTAL=${RUSTOK_MIGRATION_SMOKE_INCREMENTAL:-0}
export RUSTOK_MIGRATION_SMOKE_REUSE_DB=${RUSTOK_MIGRATION_SMOKE_REUSE_DB:-0}

if ! [[ "$RUSTOK_MIGRATION_SMOKE_DB_NAME" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
  echo "Invalid RUSTOK_MIGRATION_SMOKE_DB_NAME '$RUSTOK_MIGRATION_SMOKE_DB_NAME'. Use letters, digits, and underscores; first char must be a letter or underscore." >&2
  exit 2
fi

if ! [[ "$RUSTOK_MIGRATION_SMOKE_ADMIN_URL" =~ ^postgres(ql)?:// ]]; then
  echo "RUSTOK_MIGRATION_SMOKE_ADMIN_URL must use postgres:// or postgresql://" >&2
  exit 2
fi

for flag in \
  RUSTOK_MIGRATION_SMOKE_INCREMENTAL \
  RUSTOK_MIGRATION_SMOKE_KEEP_DB \
  RUSTOK_MIGRATION_SMOKE_REUSE_DB
do
  value=${!flag}
  if [[ "$value" != "0" && "$value" != "1" ]]; then
    echo "$flag must be 0 or 1" >&2
    exit 2
  fi
done

mode="fresh-apply-all"
if [[ "$RUSTOK_MIGRATION_SMOKE_INCREMENTAL" == "1" ]]; then
  mode="fresh-incremental"
fi
if [[ "$RUSTOK_MIGRATION_SMOKE_REUSE_DB" == "1" ]]; then
  mode="reuse-upgrade-all"
  if [[ "$RUSTOK_MIGRATION_SMOKE_INCREMENTAL" == "1" ]]; then
    mode="reuse-upgrade-incremental"
  fi
fi

echo "Running PostgreSQL migration smoke ($mode) against database '$RUSTOK_MIGRATION_SMOKE_DB_NAME'"
(
  cd "$ROOT_DIR"
  cargo test -p migration --test postgres_zero_migration_smoke \
    postgres_zero_migration_smoke_applies_from_empty_database -- --ignored --nocapture
)

echo "Migration smoke completed successfully for '$RUSTOK_MIGRATION_SMOKE_DB_NAME'"
