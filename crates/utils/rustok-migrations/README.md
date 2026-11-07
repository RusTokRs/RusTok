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

`export-migration-plan --output FILE` exports the same validated composition
used by `Migrator`. Its `schema_version: 1` and ordered `migrations` name list
remain available to append-only and backfill checks. Ordered `entries` add
the platform/module owner, declared `after` edges, optional owner safety/phase
metadata, and the exact `use_transaction` declaration (`null`, `true`, or
`false`). Both projections come from one retained executable inventory;
module ownership comes from `RusToKModule::slug()`, without another registry.
`ModuleMigrationSource::new` derives that slug by static dispatch and retains
only the narrow `MigrationSource` port. The operational inventory does not store
full module lifecycle/runtime-extension vtables. This does not remove the
composition's Cargo dependency tree; binary-size effects remain unmeasured.

Composition rejects duplicate identities/owners, foreign or absent declaration
subjects, duplicate safety/dependency declarations and edges, missing dependency
targets, and cycles. Cross-owner dependency targets remain supported. Missing
safety metadata is explicit `null`, never additive-safe evidence. The exporter
and operations CLI propagate typed composition errors before database work.
SeaORM's infallible factory retains its static compiled-composition assertion.
Ownership identifies the contributing migration source; it does not infer
table ownership, confine SQL effects, or confer module authority.

This is a diagnostic declaration inventory. It does not hash source inputs,
resolve transitive build provenance, certify observed safety, or seal a release.
The existing `xtask migrations verify-fresh` consumer binds the exported bytes
and runner/exporter digests to its source snapshot, lockfile, configuration and
normalized Cargo normal/build resolution. Source collection stays in `xtask`;
the exporter does not assert its own Git/build provenance. Workspace feature
unification and external/generated/native inputs remain explicitly incomplete
in that partial report. See the [orchestration contract](../../../xtask/README.md#migration-consolidation-commands).
The existing migration infrastructure approval check also protects the extracted
inventory implementation and its regression source, alongside the exporter.

The neutral library check (`cargo test --locked --offline -p rustok-migrations
--no-default-features --lib`) passed 24 tests, including all eight inventory
regressions. The isolated PostgreSQL observer test remained ignored.

After the Rig 0.46 adapter update, the default composition passed 11
inventory-related tests and four targeted owner coverage and RBAC repair tests.
The saved pre-cutover export and the new `export-migration-plan` output match
all 581 names in exact execution order. The new entries cover 38 platform
migrations and 543 migrations from 43 modules. The exporter was built with
`rustok-outbox/test-transport-fallback` to reuse the test build's feature graph.
This verifies inventory identity and order. PostgreSQL equivalence and release
admission require their own evidence.

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

### Bounded execution

`StepRunner::connect_phase(url, complete_plan, ExecutionPhase)` restricts that
same executor to a nonempty, contiguous phase. The request names the complete
ordered `expected_prefix` and the exact ordered `steps`; their concatenation
must match the beginning of the full compiled composition. The journal must
end inside the phase, including its prerequisite and completed boundaries.
Missing prerequisites and history beyond the end are rejected before journal
installation. Pending steps and execution stop at the terminal boundary even
when later steps exist. Restart revalidates the same phase and committed prefix.
Nontransactional steps outside the selected phase are not executed and do not
prevent this phase from running.

The CLI uses this same path:

```json
{"expected_prefix":["create"],"steps":["backfill"]}
```

```powershell
rustok-migrate up --phase phase.json
```

Names above are illustrative; use the exact compiled identities. Both fields
are required, unknown fields are rejected, and the file is limited to 1 MiB.
Phase completion does not mean that the full schema is current; use `status`
for that check. Bare `up` preserves full-plan installation/verification behavior.
See the [bounded execution decision](../../../DECISIONS/2026-11-05-bounded-migration-execution.md).

This request restricts execution; it grants no release or module authority.
Production updates still require owner admission, observed-schema/source/build
binding, fences and durable operation receipts. SQL side effects remain owned
by each migration. The installer's sparse bootstrap entrypoints remain separate.

### Verification evidence

The bounded-phase implementation has scoped execution evidence on PostgreSQL
16.13: `stepwise_recovery` passed all four tests with `--include-ignored`.
The fixture exercises exact terminal stops, committed-prefix restart, completed
retry, missing prerequisites without journal creation, history beyond the phase,
transaction failure, cancellation, backend/process loss, competing writers and
journal drift. It also asserts that unselected suffix effects remain absent.
The independent library passed 16 tests (one observer test ignored), and the CLI
passed three parser/input-bound tests. These finite scenarios do not certify
every production migration or module transition.

`cargo clippy --locked -p rustok-migrations --no-default-features --lib --test
stepwise_recovery -- -D warnings` and the composed CLI check with `--no-deps`
passed. An earlier composed dependency-lint run failed on Commerce and Forum
diagnostics before subsequent main integration. The owner checks above do not
claim a fresh lint result for every dependency.

On 2026-11-06, the built CLI was exercised against an isolated PostgreSQL
16.13 cluster using the exported 581-identity composition from
`main@74ab78c207383612a1e7103140496644a456a329`. Two successive one-step phases
applied exactly the first two identities. A completed retry emitted no step
confirmation, `status` still reported pending migrations, and the third step's
`sessions` table remained absent. Missing prerequisites were rejected without
creating the journal; a journal beyond the requested end was also rejected.
The exact final journal contained only the selected two identities. The fixture
database was removed and the temporary cluster was stopped. This is a bounded
CLI integration check, not a fresh replay of all 581 migrations or a production
admission/recovery certification.

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

The executable inventory contains owner declarations only; most migrations
currently have no safety metadata. Trusted immutable source/build coverage,
stable release seals and production authorization remain unimplemented.
The composed inventory regression and pre/post exporter comparison are blocked
by the AI Rig adapter compilation errors described above; this is not a passing
composition check.

The stepwise runner currently supports PostgreSQL transactional execution only;
selected phase steps must all support its recovery policy.
Nontransactional recovery, complete owner assertions at every retained boundary,
released-prefix/installer coverage and trusted replacement admission remain
pending in the [canonical plan](../../../docs/guides/safe-database-migrations-and-squashing.md).
Fresh-session restart tests cover committed boundaries; they do not establish
process-crash recovery for every production migration or external side effect.
