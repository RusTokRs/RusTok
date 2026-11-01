# rustok-migrations — Crate API

Neutral platform schema-composition crate.

## Public surface

- `Migrator` — the canonical platform migration chain.
- `SqliteTestMigrator` — SQLite-only migrator for portable unit tests.
- `sqlite_test_migration_is_compatible(name)` — predicate for the SQLite-compatible subset.
- `apply_installer_owner_schema(db)` / `apply_installer_remaining_schema(db)` — installer-owned schema application.
- `schema_diff` module and its root re-exports: `SchemaOperation`, `ColumnSummary`, `LockClassification`, `MigrationSafetyClassification`, `MigrationDryRunReceipt`, `MigrationDiffError`, `classify_operation`, `classify_operations`, `compute_migration_plan_digest`, `compute_schema_state_digest`, `generate_migration_dry_run_receipt`.

## Invariants

- The crate composes platform and module migration sources; it owns no domain data semantics.
- PostgreSQL-only migrations are excluded from `SqliteTestMigrator` instead of being silently approximated on SQLite.
- The installer owner prefix ends at the owner release ledger; no tenant seed or runtime traffic schema is mutated before the signed base distribution is imported.
- Schema-diff classification is advisory metadata for operator tooling and never gates the migration chain itself.
