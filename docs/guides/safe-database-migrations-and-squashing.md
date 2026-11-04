---
id: doc://docs/guides/safe-database-migrations-and-squashing.md
doc_type: canonical_guide
status: current
owner: platform-architecture
canonical_for:
  - database-migrations-squashing
language: markdown
---

# Safe Database Migrations and Squashing Guide

This guide owns the migration consolidation protocol and implementation sequence.
The [technical ADR](../../DECISIONS/2026-11-03-migration-consolidation.md) is
accepted and implementation is in progress. Rust source planning, PostgreSQL
observation and fresh-install comparison are implemented. Keep migration history
append-only until all implementation gates pass and the maintainer explicitly
admits a reviewed replacement. CI requires append-only PR migration plans.

## Stable-release policy

Every stable release freezes its published migration identities, source inputs
and execution contracts, including patch releases such as `1.2.1`. Subsequent
releases retain that history and add new migrations. A release with no schema or
data changes needs no migration.

Between releases, unpublished migrations may be amended or consolidated after
their complete schema, data and execution effects have been verified. Keep
multiple steps when dependencies, activation phases or recovery require them.
The latest supported stable release boundary and the current `main` commit are
different inputs.

Prereleases are restricted to disposable development/test databases. Valuable
persisted state requires an explicit preservation/transfer plan even when its
installation is called beta. Consolidation never grants permission to erase
that state. Stable native module publications also freeze their migrations,
independently of the host application's version.

For example, migrations shipped in `1.2.0` remain immutable while twenty new
draft migrations are reduced to a verified transition for `1.2.1`. Publishing
`1.2.1` freezes that transition as well. A first-release baseline can consolidate
the entire unpublished history after checking data disposability and installer
ordering.

## Current implementation and evidence

The initial assessment used
`main@4aeefe99ef4eaa59b9731c5d51702789a9cadc7b` on 2026-11-03. The table records
the current implementation after the first Rust tooling changes.

| Component | Existing behavior | Verification limit |
|---|---|---|
| [Source scanner](../../scripts/dev/safe-migration-squash.mjs) | Recursively inspects recognized entity/DDL shapes, file/directory entrypoints and dated references; rejects empty coverage | Regex parsing still has incomplete syntax/type coverage and warning-only cases; explicitly diagnostic |
| [Plan exporter](../../crates/utils/rustok-migrations/src/bin/export_migration_plan.rs) | Exports ordered runtime migration names | No migration source hashes, stable seal or independent profile selection |
| [Plan comparator](../../scripts/verify/verify-migration-plan-compatibility.mjs) | Requires the PR base name list to remain an exact prefix | Unchanged names can conceal changed bodies/helpers |
| [Migration Compatibility](../../.github/workflows/migration-compatibility.yml) | Wires fresh/incremental PostgreSQL, last-step rollback and PR-base upgrade; exporter target matches Cargo | PR base is not the previous stable release |
| [Rust workflow](../../xtask/src/migrations.rs) | Snapshots exact original Git blobs and current candidate inputs; builds isolated source copies and binds fresh-install reports to source/tool/binary identities | Local fresh-install evidence; no trusted release/range/recovery admission |
| [PostgreSQL observer](../../crates/utils/rustok-migrations/src/verification.rs) | Compares real schema, privileges, enforcement, sequences, row multisets and each plan's journal | PostgreSQL 16, supported fixture limits and extensions; requires quiescent writers and complete read privileges |
| [Backfill fixtures](../../crates/utils/rustok-migrations/tests/support/backfill_fixtures.rs) | Runs declared setup and strict boolean assertions | One case per migration, all setup precedes the pending range |
| [Schema summaries](../../crates/utils/rustok-migrations/src/schema_diff.rs) | Classifies supplied operations and computes summary/plan digests | No actual PostgreSQL catalog observation |
| [Release workflow](../../.github/workflows/release.yml) | Validates signed tags, immutable releases and artifact provenance | No sealed migration manifest or exact-release migration evidence gate |

The initial scanner run produced six passes and one failure. Blog's nine
entities were reported missing because DDL lives in nested Rust files. Recursive
source inspection, directory `mod.rs` entries and eight-digit date matching are
now handled. Empty inventories fail. Runtime output describes recognized source
diagnostics and never claims complete schema parity. Field SQL types,
constraint/index/trigger/RLS equivalence and arbitrary Rust/SQL syntax still
require execution-based verification.

The workflow's incorrect `--bin export_migration_plan` calls were changed to
`export-migration-plan`. The contract guard now checks both calls against the
Cargo target and forbids the previous spelling. This structural regression
check supplements actual execution evidence.

These limitations must be resolved before source diagnostics or CI results can
admit a consolidation. The checks below remain useful for bounded investigation.

```bash
npm run verify:schema-parity -- --module <module-slug>
npm run verify:schema-parity:all
npm run test:verify:schema-parity
```

The current `--all` scanner visits only module directories with both
`src/entities` and `src/migrations`. It omits other layouts and platform-owned
migrations. Empty scans are not schema coverage.

## Verification workflow

### Rust entrypoint and ownership

The repository command group is `cargo xtask migrations`. `xtask` orchestrates inventory/planning, source
snapshots, candidate previews, scoped owner processes and reports.
`rustok-migrations` implements actual PostgreSQL observation, migration execution
and typed schema/data comparisons. PostgreSQL dump/restore tools run as pinned
subprocesses. The implemented commands are `plan` and `verify-fresh`; see the
[xtask command contract](../../xtask/README.md#migration-consolidation-commands)
for arguments and the [owner tool contract](../../crates/utils/rustok-migrations/README.md#verification-tool)
for limits and explicit PostgreSQL regression invocation.

The owner process boundary keeps the complete domain migration dependency tree
out of ordinary `xtask` compilation. Candidate source edits use the current
working branch. Reference execution uses immutable source snapshots in
disposable directories without creating branches. The Rust workflow replaces
the existing JavaScript source-squash entrypoint when its gates are complete.

### Capture immutable inputs

Pin the released predecessor B, original draft O and candidate C by full source
identity. Before the first stable release, B is the explicitly recorded empty
bootstrap boundary. Record composition, features, dependency lock/toolchain,
PostgreSQL image/version/extensions, locale/collation, role policy and fixture
hashes. Fetch protected release evidence through trusted release tooling.
Keep all published stable seals protected even when their releases leave the
supported upgrade matrix. Select O through trusted admission from the complete
protected draft source or a separately reviewed preparatory commit.

Create candidate files in the current working branch. Preserve reproducible original
source and existing databases while preparing and verifying the change. Any
source, fixture, policy, environment or release-boundary change invalidates the
report. Recheck `main` and stable seals before integration.

### Inventory and map every removed migration

Derive actual order and metadata from `MigrationSource` and the composed
`Migrator`. For every replaced migration, record its owner, replacement and the
disposition of schema objects, initial data, transformations, integrity guards,
outbox/receipt/work effects and phase/recovery constraints. Resolve cross-owner
dependencies and all consumers of exact migration names.

Installer bootstrap is a separate required path. `InstallerOwnerMigrator` and
`apply_installer_owner_schema` select exact `rustok-modules` identities before
base-distribution import. A globally valid migration list alone does not verify
this prefix. Test bootstrap/import/remaining-schema ordering explicitly.

### Build the candidate

Retain the immutable published history and consolidate only the declared draft
range. Write the owner migration using existing SeaORM/SeaQuery and owner SQL
contracts. Actual database inventory provides a review reference. Data effects
remain explicit typed transformations or reviewed owner SQL.

Preserve necessary dependency cuts. For `A1 -> B1 -> A2`, merging both A steps
creates a cycle. Preserve pre/post-activation, maintenance, backfill, concurrent
DDL and recovery boundaries even when the final tables would look identical.
Validate the candidate's global DAG and exact phase metadata before execution.
Missing safety metadata requires owner review and blocks admission. It must
never be interpreted as additive safety or permission to run in any phase.

### Execute both paths

Run O and C from empty PostgreSQL databases through their real production
composition. Then install B once, load deterministic owner fixtures, capture a
consistent quiescent snapshot and restore it into two separate databases. Apply
the original draft tail to one and the candidate tail to the other.

The second comparison is mandatory after stable publication. Empty databases
cannot expose loss of existing business rows or backfills. For the first
baseline, verify all required initial data and any explicit retained-state
transfer separately. Do not seed a removed intermediate schema into B unless
that checkpoint is itself retained and tested.

### Compare observed results and expected behavior

Compare the actual PostgreSQL catalog inventories defined in the ADR, including
database integrity and security objects. Record unknown/unreadable objects as
failures. Compare each migration journal with its own expected plan. Preserve
the journal table's schema and verify no unknown/pending migrations remain.

Inventory and compare every persistent application table, including undeclared
owners that a trigger or raw SQL might mutate. Compare canonical row values and
references, required initial data, sequence state, receipts and outbox/work state.
Every journal/derived/nondeterministic exclusion requires an owner-reviewed
disposition. Use multiset comparison when rows have no stable identity.
Use owner-defined ordering and typed serialization. Counts/checksums alone cannot
explain a mismatch. Restrict nondeterminism handling to explicitly reviewed
fields and retain independent expected-value assertions on both paths.

Exercise owner reads/writes, invalid and cross-tenant rejection, uniqueness,
relevant projections and recovery. Test RLS through the intended application
roles. A catalog match does not establish that a service query or authorization
path behaves correctly. Rollback tests cover only explicitly reversible steps.
Irreversible steps require a restore/forward-recovery rehearsal.

### Admit and seal

A trusted base-owned verifier checks the declared range, protected history and
complete report. Infrastructure approval alone cannot authorize rewriting
published history or skipping equivalence. Validate the current release
boundary and exact candidate identities again at integration.
Serialize final integration and stable publication through trusted admission
against one monotonic seal generation, or an equivalent atomic fence. Bind final
evidence to the merge-result source/build identity. A separate pre-merge check
cannot close the release-publication race.

Before admitting deletion of draft source, persist the content-addressed O
source/build inputs and reports, verify their digests and retrieval, and bind
them to the admission record. Verify actual required-check settings and release
job dependencies so missing/skipped checks cannot satisfy the gate.

Stable release publication checks evidence for its exact commit, then publishes
and attests the derived migration manifest. Keep it and the source/evidence
archives under release retention, beyond ordinary short-lived CI artifacts.
Existing exact asset lists, checksum generation, supply-chain tests and protected
infrastructure rules must be updated together when that asset is introduced.
Reserve the stable identity before exposing any stable artifact, including a
container image. A failed publication after exposure retains the protected
reservation and never returns its migrations to draft status.

## Implementation gates

This is the single ordered consolidation work plan. Each gate needs executable evidence before the
next admission capability becomes available.

1. **Restore trustworthy existing checks.** Fix the workflow binary target,
   nested-source/directory entrypoint inventory and reference-date matching.
   Reject empty or unsupported coverage explicitly. Remove misleading complete
   parity claims from runtime output. Add adversarial checks for
   the observed false positives/negatives and report precise supported scope.
   Run the existing composed PostgreSQL smoke on a pinned source/environment.
   Establish the Rust `xtask` orchestration and scoped owner-tool boundary. Keep
   the current diagnostics clearly scoped until the Rust cutover is verified.
2. **Implement protected release identity.** Extend the canonical exporter with
   owner/dependency/safety and source-input identity. Derive hashes from immutable
   Git trees using trusted code and resolve complete transitive source/build
   input coverage. Prepare generated manifests through existing release tooling
   and preserve the explicit first-release bootstrap state. The first
   authoritative stable seal requires the exact-commit evidence from later
   gates. Bind replay to dependency/build provenance. Do not yet admit
   replacement ranges. Specify the atomic integration/publication fence and
   collect reviewed constraints where existing safety metadata is absent.
3. **Observe and compare real databases.** Add typed catalog observation and
   independent schema-dump diagnosis in `rustok-migrations`. Reuse isolated
   PostgreSQL database lifecycle support. Cover first-install, released-prefix
   replay and installer bootstrap. Reject missing or unsupported inventory.
4. **Verify data and recovery for ranges.** Extend existing backfill contracts
   for multiple predecessor/range cases and independent target assertions.
   Restore one seeded predecessor snapshot into both branches. Compare canonical
   data and runtime invariants and rehearse interruption/retry/recovery at every
   retained boundary. Preserve phase and cross-owner dependency constraints.
5. **Wire controlled admission and validate a pilot.** Only after gates 1–4,
   introduce the protected draft-replacement CI mode. Preserve infrastructure
   trust/isolation and require all evidence at exact integration/release commits.
   Verify actual required-check settings, publication ordering and durable
   reference/report retrieval. Missing or skipped checks block admission.
   Pilot with a bounded owner having representative data and constraints, then
   expand to dependent owners. Blog can exercise fresh-baseline nested-source
   handling, but a post-release upgrade pilot also needs a real predecessor.

`verify-fresh` replays each migration separately. The executor reads the ordered
plan and journal prefix before applying steps. It confirms each committed
journal entry before advancing. The verifier checks the full sequence of
confirmations against the executable plan for each fixture, then independently
checks its final journal and database state. A replacement still requires an
independent review of every removed migration and its disposition.

Current status is implementation in progress. Existing-check corrections, Rust
source planning and the bounded fresh-install observer/runner are implemented.
On PostgreSQL 16.13, the composed runner applied and confirmed all 580 exported
identities one at a time on each of two empty fixtures. Both confirmation
sequences and final journals matched the executable plan. The schema observations
agreed. The existing fresh PostgreSQL smoke test also passed on this cluster,
including its independent schema assertions.

The strict data comparison remains blocked by `updated_at` in eight bootstrap
singletons. The affected owner tables are:

- Channel: `channel_resolution_invalidation_state`.
- Flex: `flex_field_definition_cache_generation`.
- Modules: `module_artifact_node_reconciliation_state`, `module_security_epoch`,
  `module_static_distribution_state`, `module_static_distribution_release_state`,
  and `module_static_distribution_rollout_state`.
- RBAC: `rbac_invalidation_state`.

The fresh report diagnoses these tables independently on each side. It checks
the expected singleton, column types, identity, counters and references, and
checks `updated_at` against the installation window measured by PostgreSQL.
The report marks a difference as clock-only only after both checks pass.
The PostgreSQL 16.13 full-chain run on 2026-11-04 passed all eight diagnostics
on both fixtures. Strict comparison still reported those eight timestamp
differences and blocked equivalence.
The window spans `up` and `status`. Per-migration clock bounds and independent
schema/data assertions at each retained boundary remain pending.
This diagnostic leaves strict comparison and its original digests in force.
An owner-reviewed disposition is required before the verifier can accept a
clock-only difference for equivalence.

Gate 3 still needs a passing fresh comparison, released-prefix replay and
installer coverage. Gates 2, 4 and 5 remain pending.
No migration history is removed by the tooling change. A production prerelease prohibition
also requires enforcement at the installer/update admission boundary. Release
workflow prerelease labels by themselves do not enforce it.

## Acceptance and verification

Consolidation is accepted only when published history is intact, every removed
migration is accounted for, the composed DAG/bootstrap/phases are valid, fresh
and supported-upgrade schema/data assertions pass, recovery is verified, and
reports bind the exact inputs and current release seals. Unknown coverage keeps
the result incomplete. Finite fixtures establish the exercised contract, not a
universal guarantee over arbitrary migrations and datasets.

Use scoped checks for each changed owner and the migration verifier.

```bash
cargo check --locked -p <crate-name>
cargo test --locked -p <crate-name> --lib
```

Run the existing PostgreSQL migration harness for the applicable composition
and declared environments. Preserve its required-environment checks so a missing
database cannot turn into a passing skipped test. Portable SQLite tests provide
additional evidence only for their supported subset.
