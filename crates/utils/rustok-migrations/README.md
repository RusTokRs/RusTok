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

`identity::migrations()` is available without default features and returns the
same shared tenant/user/session/authorization creation migrations consumed by
the production composition. Scoped fixtures use it to satisfy actual foreign
keys instead of recreating foundation tables. It is a creation prefix, not a
replacement for later owner migrations or the production `Migrator`.

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

## Shared migration assessment

`schema_diff` and its supplied-operation classifier are available independently
of `composition`, alongside the PostgreSQL observer. They consume the canonical
`rustok_core::MigrationSafetyAssessment`; the module control plane consumes that
same fact type without importing this crate or its domain composition.

Maintenance and unknown assessments cannot become safe through missing reason
text. Locking foreign keys retain diagnostics, opaque raw SQL and empty operation
coverage remain unknown. Adding a column requires maintenance because the
summary does not prove `ACCESS EXCLUSIVE` lock duration or default/rewrite safety;
concurrent index creation requires nontransactional recovery evidence. The assessment is
the source of truth; there is no separate automatic-eligibility boolean on
`MigrationDryRunReceipt`. Schema/plan summary hashes encode array boundaries.
The target schema digest is still a projection, not an observed database digest.
These summaries grant neither deployment nor consolidation admission.

```powershell
cargo test --locked -p rustok-migrations --no-default-features --lib schema_diff
```

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

`verify-migrations fresh` runs each `rustok-migrate up/status` binary from its
source root against a newly created fixture database and login. It observes and
removes the original fixture before creating the candidate fixture. Both use
the same generated names so ownership and grants can be compared directly.
`up` uses `execution::StepRunner`, which loads the plan once and accepts only a
duplicate-free journal matching a prefix of that plan. It applies one migration
at a time and checks the complete committed journal prefix before emitting a
confirmation. A dedicated single-connection PostgreSQL pool holds the session
advisory lock on the execution backend itself. Connection replacement is
forbidden: session loss cannot leave a migration running on an unlocked backend
or silently reconnect midway through a step. A failed or cancelled step poisons
the runner; callers close it before reconciling through a new session.
Plans containing nontransactional steps are rejected before connecting until
their owner-proven recovery contract is supported. No step is implicitly made
transactional.
`status` reads the journal without creating it. Fresh verification requires
the ordered confirmations from both runners and checks each final journal
against its executable plan.

Fresh reports include bootstrap diagnostics for eight known singleton tables.
The verifier uses PostgreSQL's clock to bound each run and checks the exact row
count, column types, keys, counters, references and other seed values. A
diagnostic flags a clock-only difference when both rows satisfy those checks and
only `updated_at` differs. The raw observations still determine equivalence.
Recreating the login with a new password also discards original role settings
before the candidate runs. Migration children
receive their fixture URL and a bounded OS environment. `compare --quiescent`
compares two already prepared fixture endpoints without creating or deleting
databases. Run `verify-migrations --help` for explicit arguments. Reports contain
digests and difference categories rather than fixture row values. A successful
comparison is scoped evidence; `replacement_admission` always remains blocked.

The PostgreSQL smoke test uses this same executor. Both modes apply individual
steps; incremental mode additionally closes and recreates the executor after
every committed step to exercise journal-based resumption. Backfill setup stays
on the predecessor, before the pending range. Every selected fixture must target
a pending migration. Its assertion runs in a read-only, repeatable-read
transaction immediately after that migration and again at the end of the range.
Missing, NULL, nonboolean or multiple assertion results fail verification.
This is not yet complete owner-assertion or crash-recovery coverage of the chain.

The ignored PostgreSQL regression is explicit about its required environment:

```powershell
$env:RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL = 'postgres://verifier@localhost:5432/postgres'
$env:RUSTOK_MIGRATION_VERIFY_TEST_PG_DUMP = 'C:/Program Files/PostgreSQL/16/bin/pg_dump.exe'
cargo test --locked -p rustok-migrations --no-default-features --lib postgres_detects_schema_data_privilege_and_enforcement_changes -- --ignored
cargo test --locked -p rustok-migrations --no-default-features --bin verify-migrations -- --include-ignored
cargo test --locked -p rustok-migrations --no-default-features --test stepwise_recovery -- --include-ignored
```

See [the database architecture](../../../docs/architecture/database.md) and the
[Axum runtime and operations CLI boundary](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md).

### Known Limitations / Pending Implementation

The stepwise runner currently supports PostgreSQL transactional plans only.
Nontransactional recovery, complete owner assertions at every retained boundary,
released-prefix/installer coverage and trusted replacement admission remain
pending in the [canonical plan](../../../docs/guides/safe-database-migrations-and-squashing.md).
Fresh-session restart tests cover committed boundaries; they do not establish
process-crash recovery for every production migration or external side effect.
