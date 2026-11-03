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

The [consolidation decision](../../../DECISIONS/2026-11-03-migration-consolidation.md)
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
ordered implementation gates. The PostgreSQL observer and fresh-install runner
are implemented; release sealing and full replacement admission remain pending.
Existing `schema_diff` operation summaries and projected target
digests do not observe a PostgreSQL database or certify consolidation safety.

Implementation must reuse the real `Migrator`, installer bootstrap entrypoints,
`MigrationSource` dependency/safety metadata and PostgreSQL fixture harness.
The current composed exporter covers the crate's compiled module set. Separate
distribution/profile selection needs executable wiring and its own evidence.

### Verification tool

The default `composition` feature preserves the complete existing `Migrator`,
installer entrypoints and exports. Build the independent observer without that
domain dependency tree:

```bash
cargo build --locked -p rustok-migrations --no-default-features --bin verify-migrations
```

`verification::observe_database` observes PostgreSQL 16 through one repeatable
read transaction and the matching exported `pg_dump` snapshot. It retains schema
definitions, ownership and privileges, database/role settings, internal trigger
states, index readiness, supported extension definitions, sequences, and exact
ordered table row multisets. Text representations preserve numeric precision,
SQL NULL, duplicates and JSON source values. Each journal must match its own
executable plan; consolidated journal identities may differ.

The observer requires complete read privileges and quiescent fixture writers.
Fixture URLs explicitly name their host, user and database and currently do not
accept query options or fragments. IPv6 and percent-encoded identities are
supported; connection fields are passed through a cleared libpq environment.
It fails on inaccessible/RLS-filtered data, unsupported extension state, foreign
tables, large objects, subscriptions, incomplete indexes, missing journals, tool
failures or limits. Current limits are 100,000 rows and 64 MiB across fixture
tables, 1 MiB per row, 64 MiB schema output, 30-second SQL statements and a
120-second dump process. Supported extensions are `plpgsql` and `pg_trgm`.

`verify-migrations fresh` creates temporary UUID-named roles/databases, invokes
the two actual `rustok-migrate up/status` binaries from their source roots,
observes each result immediately, compares, and cleans up. Migration children
receive their fixture URL and a bounded OS environment. `compare --quiescent`
compares two already prepared fixture endpoints without creating or deleting
databases. Run `verify-migrations --help` for explicit arguments. Reports contain
digests and difference categories rather than fixture row values. A successful
comparison is scoped evidence; `replacement_admission` always remains blocked.

The ignored PostgreSQL regression is explicit about its required environment:

```powershell
$env:RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL = 'postgres://verifier@localhost:5432/postgres'
$env:RUSTOK_MIGRATION_VERIFY_TEST_PG_DUMP = 'C:/Program Files/PostgreSQL/16/bin/pg_dump.exe'
cargo test --locked -p rustok-migrations --no-default-features --lib postgres_detects_schema_data_privilege_and_enforcement_changes -- --ignored
cargo test --locked -p rustok-migrations --no-default-features --bin verify-migrations -- --include-ignored
```

See [the database architecture](../../../docs/architecture/database.md) and the
[Axum runtime and operations CLI boundary](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md).
