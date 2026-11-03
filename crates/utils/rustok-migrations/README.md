# rustok-migrations

Public surface and contract invariants are documented in [CRATE_API.md](CRATE_API.md).

## Purpose

`rustok-migrations` is the neutral platform schema-composition crate.

It combines the platform-owned SeaORM migrations with migration sources
exported by selected domain modules and validates declared cross-module
ordering.

## Responsibilities

- Compose platform and selected module migration sources into one ordered
  `Migrator`.
- Validate declared cross-module migration dependencies.
- Keep schema composition independent from HTTP routing and executable hosts.

## Interactions

- `rustok-installer-persistence` applies `Migrator` once for the shared
  installation database.
- `rustok-installer-cli`, selected by `rustok-cli`, exposes `migrate up` and
  `migrate status` without importing `apps/server`.
- Domain modules export their own `MigrationSource` implementations; this crate
  aggregates them but does not own their tables or migration logic.

The crate does not depend on `apps/server`. The HTTP host owns request runtime
only; operational schema changes are invoked through the platform CLI or the
installer workflow.

## SQLite unit-test schema

`Migrator` is the only production schema composition path and always includes
every selected migration. `SqliteTestMigrator` is restricted to repository unit
tests that exercise portable service behavior on SQLite. It excludes the
explicitly PostgreSQL-native migrations that depend on PostgreSQL functions,
triggers, enum semantics, or constraints; those migrations remain covered by
PostgreSQL integration tests.

Narrow SQLite component fixtures must use
`sqlite_test_migration_is_compatible` when selecting their own migration
subset. This is a test-environment boundary, not a deployment fallback or a
second production schema contract.

## Planned Module Release Safety Integration

The
[module release and rollback plan](../../../docs/modules/module-release-rollback-plan.md)
keeps this crate as the trusted neutral migration executor.
`rustok-modules` owns update mode, the exact digest-bound migration phase plan,
fences, checkpoints, and recovery outcome; `rustok-migrations` validates and
executes only that authorized phase and returns an idempotent receipt. It does
not choose a rollback target, restore policy, or finalization time.

The initial installer may still apply the complete selected schema. A
production module update must not invoke an unbounded “up to latest” migrator:
it executes only the approved additive, backfill, or separately authorized
finalization phase. Process loss and unknown executor outcomes must reconcile
against the exact migration identity before any retry.

## Entry point

- `rustok_migrations::Migrator`

## Migration consolidation design

The [consolidation proposal](../../../DECISIONS/2026-11-03-migration-consolidation.md)
extends this crate's existing composition and execution responsibility with
PostgreSQL catalog observation and isolated reference/candidate verification.
Domain owners retain migration code, data transformations and independent
invariant assertions. Release tooling owns immutable publication evidence.

The developer workflow is implemented in Rust behind `cargo xtask migrations`.
`xtask` orchestrates source snapshots, candidate files and reports, while this
crate owns database observation, schema/data comparison and migration execution.
Use scoped owner tools through a process boundary to keep `xtask` independent
from this crate's complete domain-module dependency set.

The [canonical guide](../../../docs/guides/safe-database-migrations-and-squashing.md)
records the agreed stable-release policy, current tooling limitations and the
ordered implementation gates. The proposed verifier and release seal are not
implemented. Existing `schema_diff` operation summaries and projected target
digests do not observe a PostgreSQL database or certify consolidation safety.

Implementation must reuse the real `Migrator`, installer bootstrap entrypoints,
`MigrationSource` dependency/safety metadata and PostgreSQL fixture harness.
The current composed exporter covers the crate's compiled module set. Separate
distribution/profile selection needs executable wiring and its own evidence.

See [the database architecture](../../../docs/architecture/database.md) and the
[Axum runtime and operations CLI boundary](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md).
