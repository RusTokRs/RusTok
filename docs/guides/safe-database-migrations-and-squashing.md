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
| [Plan exporter](../../crates/utils/rustok-migrations/src/bin/export_migration_plan.rs) | Exports ordered runtime migration names and owner/dependency/safety/transaction declarations from the executable inventory | No migration source hashes, stable seal or independent profile selection |
| [Plan comparator](../../scripts/verify/verify-migration-plan-compatibility.mjs) | Checks the PR base name list and can compare executable declarations against a supplied protected prefix | The protected prefix must come from trusted release tooling. Unchanged declarations can conceal changed bodies/helpers. |
| [Migration Compatibility](../../.github/workflows/migration-compatibility.yml) | Wires fresh/incremental PostgreSQL, last-step rollback and PR-base upgrade; exporter target matches Cargo | PR base is not the previous stable release |
| [Rust workflow](../../xtask/src/migrations.rs) | Snapshots exact original Git blobs and current candidate inputs; builds isolated source copies and binds fresh-install reports to source/tool/binary identities | Local fresh-install evidence; no trusted release/range/recovery admission |
| [PostgreSQL observer](../../crates/utils/rustok-migrations/src/verification.rs) | Compares real schema, privileges, enforcement, sequences, row multisets and each plan's journal | PostgreSQL 16, supported fixture limits and extensions; requires quiescent writers and complete read privileges |
| [Backfill fixtures](../../crates/utils/rustok-migrations/tests/support/backfill_fixtures.rs) | Validates pending targets, seeds the predecessor and runs read-only strict boolean assertions at each selected target and at range completion | One case per migration; full range/multiple-case and owner coverage remain pending |
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
subprocesses. The Rust workflow provides planning, fresh comparison, source
fingerprinting, source discovery and executable inventory binding. See the
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

Installer bootstrap is a separate required path. `apply_installer_owner_schema`
selects required platform and `rustok-modules` steps from the composed plan before
base-distribution import. It validates the full journal and limits application
to this selection. Verification covers the bootstrap, import, and remaining
schema boundaries explicitly.

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

The [release history resolver](../../scripts/release/resolve-release-history.mjs)
reads every page of the official `RusTokRs/RusTok` GitHub Releases history. It
reports the release ID, tag object and exact commit for each published stable
release after checking immutability and GitHub's annotated-tag signature
verification. It uses the same tag syntax as release validation. It fails on
an empty history, inconsistent metadata or any unavailable release identity.

```sh
node scripts/release/resolve-release-history.mjs
```

The [release manifest verifier](../../scripts/release/verify-release-manifests.mjs)
uses the same history to fetch each `release-manifest.json`. It verifies both
asset membership in the immutable release and provenance from `release.yml`
for the signed tag and commit. It then matches the manifest's repository, tag,
version and commit and reports its SHA-256 and artifact index. The command
requires an authenticated GitHub CLI with `release verify-asset` and
`attestation verify` support. Provide an existing scratch directory owned by
the verifier.

```sh
node scripts/release/verify-release-manifests.mjs --scratch target
```

The released migration boundary still requires attested migration manifests
and their source-input hashes, independent module seals and any stable
reservations made before a container was exposed. An empty GitHub history
cannot substitute for an explicit bootstrap record.

The first stable distribution starts from the [bootstrap record](../../scripts/release/migration-bootstrap.json)
at generation zero. `node scripts/release/resolve-release-history.mjs --check-bootstrap`
reads that exact path from a pinned commit on the official `main`, verifies
its Git blob identity and checks GitHub Release pages, repository tag pages,
the GHCR tag list and migration reservation refs for the official distribution.
A stable version tag, stable image alias such as `latest`, or migration
reservation stops the check even when there is no GitHub Release. The result
binds the observations to the commit and blob, with migration replacement
still blocked.

`node scripts/release/resolve-release-history.mjs --check-container-history`
lists the stable tags seen in GHCR independently of the bootstrap record.
Both commands authenticate to GHCR using `GITHUB_ACTOR` and `GHCR_TOKEN`
or the workflow token `GITHUB_TOKEN` with package read access. A failed tag
list request, including HTTP 404, leaves container history unverified. The
list covers current tags. Earlier exposures with deleted tags require historical
evidence. Independent module seals require separate evidence.
Release tooling tests exercise the GHCR authentication exchange, short pages
with continuation links, full pages without links, stable aliases and failed
reads. The live check still needs a credential with access to the package.

`node scripts/release/resolve-release-history.mjs --check-migration-reservations`
reads the reservation history directly from the official repository's
`refs/tags/rustok-migration-seal-<generation>` namespace. The reader works with
reservations whose ordinary release tag, release page and image are absent.
Each positive generation names an annotated tag targeting the exact release
commit. Its compact UTF-8 JSON message starts with `schema_version` (1),
`repository` (`RusTokRs/RusTok`), `seal_generation`, and `previous_tag_object`
(null for generation one). The remaining fields are `scope`
(`stable_distribution`), `release_tag` (stable SemVer), `source_commit`, and
`migration_manifest_sha256`, in that order. The reader verifies the Git tag target,
consecutive generations, predecessor links and unique platform release tags.
It currently accepts the stable distribution scope. Independent native module
reservations must participate in the same generation sequence before promotion
to an admission boundary. Fixtures cover a reservation without publication,
corrupt links, duplicate and missing generations, invalid digests and failed
reads. An empty observation leaves replacement admission blocked.

A local declaration check takes the ordered inventory projection of one
composition's protected prefix and a full candidate export.

```sh
node scripts/verify/verify-migration-plan-compatibility.mjs \
  --protected-prefix protected-inventory.json --head candidate-inventory.json
```

Both inputs require version 1 `entries` aligned with `migrations`. The
protected inventory must be nonempty. A candidate can replace draft entries
after that boundary while retaining protected names, execution order, owners,
dependency edges, safety and phase metadata, and transaction declarations.
`null` is preserved as a declaration value. The current PR check still uses
`--base` and requires the entire base name list to remain an exact prefix.
The local check compares supplied plans. The separate [`xtask source-inputs`
command](../../xtask/README.md#migration-consolidation-commands) reuses the Git
snapshot reader and compares the exact paths, file modes and bytes of a
reviewed list of migration inputs across two commits. It detects changes to
selected nested Rust helpers and SQL assets. Given explicit Rust entrypoints,
it checks that the declaration contains every file reached through supported
Rust module declarations and literal includes. This checks local syntax.
The [`bind-inventory` command](../../xtask/README.md#migration-consolidation-commands)
joins these source results to the ordered executable export validated by a
completed `verify-fresh` run. It requires the build snapshot to match a Git
commit and keeps uncovered migration rows visible in the report.
The same workflow can generate reviewable declarations from the executable
inventory, Cargo's saved dep-info paths, and the committed Rust name and
module declarations. Ambiguous names and unsupported local links require
review before the generated declarations are used.
New `verify-fresh` reports bind the raw exporter dep-info and its normalized
workspace path list to the same build identity as the exporter and inventory.
`bind-inventory` can verify that retained list and reject declarations for
files absent from the selected compiler observation. This recheck uses files
from the completed run and needs no PostgreSQL replay.
The `registry-inputs` command also reuses that build evidence to check the
selected crates.io cache archives against the committed `Cargo.lock` and
the extracted files against those archives. It records the cache bytes
observed during verification. Exact external inputs used during a build and
non-crates.io sources still require separate provenance.
For a new `verify-fresh` run, `--registry-cache` checks the cache at both
ends of each build and verifies that Cargo resolved the package manifests
from that extraction. The selected build identity then includes the
matching crate and file digests. This establishes endpoint observations
for crates.io packages. New builds also capture the regular files in every
Cargo-reported `OUT_DIR` and recheck those paths, modes and bytes before the
run's report is published. Exact compiler reads, generated files outside
these directories and transient inputs still require separate evidence.
The report links captured files to the runner and exporter through each
executable's Cargo dep-info. A listed `OUT_DIR` file missing from the capture
fails verification. Other reported paths inside the build target are visible
in `uncaptured_target_paths`.
Trusted release tooling still needs to prove that the list covers all build
and execution inputs, authenticate and select the boundary,
cover independent module seals, and authorize any draft replacement.

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
For the Git reservation namespace, trusted publication must atomically create
the next generation ref and allow replay only for the same reservation. GitHub
protection must restrict creation to the trusted writer and independently
prevent updates and deletions, including by that writer. The reader currently
checks the stored structure. The trusted writer, active ref protection, exact
manifest evidence, and shared integration/publication fence remain required
before the records can authorize an admission decision.

## Implementation gates

The [shared assessment contract](../../DECISIONS/2026-11-05-migration-safety-assessment.md)
is a bounded foundation for gate 2: schema summaries and module preflight share
`rustok_core::MigrationSafetyAssessment`, with explicit maintenance and unknown
states. Empty diagnostics cannot assert safety. It does not collect missing
owner metadata, observe desired/current production schemas, seal releases or
complete gates 2–5. `schema_diff` remains diagnostic and its target digest remains
projected; consolidation replacement admission stays blocked.

This is the single ordered consolidation work plan. Each gate needs executable evidence before the
next admission capability becomes available.

Gate 2 now has one executable declaration inventory in `rustok-migrations`:
`Migrator::inventory()` composes the same retained objects used by execution
and exports their owners, dependency edges, optional safety/phase declarations,
and exact transaction policy. The source list also includes Iggy Connector;
module ownership comes from the owner trait. The existing ordered name list is
a derived projection for append-only/backfill consumers. Missing declarations
remain unknown. Immutable Git/build input coverage, stable seals and their
atomic integration/publication fence remain pending; this inventory completes
neither gate 2 nor production update authorization.

The neutral library passed 24 Rust tests, including eight inventory regressions;
the isolated PostgreSQL observer test remained ignored. After the Rig 0.46
adapter update, the composed library passed 11 inventory-related tests and four
targeted owner coverage and RBAC repair tests. A new exporter built with
`rustok-outbox/test-transport-fallback` matched all 581 pre-cutover migration names
in exact execution order. Its entry projection contains 38 platform migrations
and 543 migrations from 43 modules. These checks establish inventory and order
parity. Database equivalence and release admission require separate evidence.

`xtask migrations verify-fresh` now binds each side's executable inventory to
the existing path/mode/blob snapshot, lockfile, Cargo/toolchain configuration,
normalized native normal/build dependency graph, tool versions, arguments and
actual binary digests. It compares resolution before and after the build.
Workspace feature unification can overstate actual compilation features;
external package bytes outside the recorded registry checks, exact generated
input consumption, build-script/proc-macro effects,
ambient native/toolchain inputs and per-migration attribution remain explicitly
unverified. This is partial build evidence, not a complete input read trace or
per-migration freeze set. The module archive `source_digest` cannot replace
workspace/build identity. The existing snapshot, publication and source-store
owners remain canonical. All 15 migration-command tests passed with
`--include-ignored`, including ten build-input regressions, real Cargo metadata
collection on an isolated workspace and a read-only resolution of the repository
at that time (1084 packages, 75 captured path packages). The Blog pilot below
exercises the complete source and database comparison workflow.

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

The shared stepwise executor binds its advisory lock to the same PostgreSQL
backend that executes migrations. It forbids connection replacement and poisons
itself after an error or cancellation. Each step validates the full journal
prefix, including duplicate rejection, before the next boundary is exposed.
Plans containing nontransactional steps are rejected before connecting until a
proven recovery contract is available; no transaction policy is silently changed.
The PostgreSQL smoke uses the same executor, with fresh-session resumption after
every committed step in incremental mode. Selected backfill assertions now run
immediately after their own step as well as at the end. Assertions are read-only;
targets outside the pending range fail before fixture setup.

The [bounded execution contract](../../DECISIONS/2026-11-05-bounded-migration-execution.md)
adds exact contiguous phases to this same runner and the operations CLI. It
checks a complete prerequisite prefix and selected step list against the full
compiled composition, then refuses journals before or beyond that phase.
Restart may resume only within the selected boundaries; no suffix step can run.
This supplies an execution primitive for retained verification boundaries. It
does not provide release seals, source/schema authorization, durable module
operation receipts or replacement admission, and does not replace the sparse
installer bootstrap path. Gates 2–5 remain incomplete.

Current status is implementation in progress. Existing-check corrections, Rust
source planning and the bounded fresh-install observer/runner are implemented.
On PostgreSQL 16.13, the composed runner applied and confirmed all 580 exported
identities one at a time on each of two empty fixtures. Both confirmation
sequences and final journals matched the executable plan. The schema observations
agreed. The existing fresh PostgreSQL smoke test also passed on this cluster,
including its independent schema assertions.

On 2026-11-09, the composed PostgreSQL 16.13 smoke applied all 586 migrations
from an empty fixture with the Forum snapshot changes. It rolled back exactly
`m20261109_000044_allow_pending_topic_snapshots`, reapplied it and verified the
schema again. Separate checks rejected an omitted admin URL and a fresh database
name collision. The existing database's sentinel row survived and the successful
smoke removed its own fixture.

On 2026-11-10, the PostgreSQL 16.13 suite passed all 13 checks. It applied 587
migrations from an empty fixture and rolled back and reapplied exactly
`m20261110_000045_reconcile_archived_counters`. It also verified bounded backfill
assertions, the current Product catalog read port, Product tenant integrity and
database constraints. The Product owner rollback rehearsal reverted and replayed
only its latest migration. The historical category tree constraints were tested
at the migration boundary where Product still owned that tree.
Separate owner tests verified counter repair for an existing archived Forum
thread, Product tenant constraints and catalog facet behavior on PostgreSQL.

The PostgreSQL 16.13 full-chain run on 2026-11-04 passed all eight diagnostics
on both fixtures. Strict comparison still reported those eight timestamp
differences and blocked equivalence. The [bootstrap clock disposition](../../DECISIONS/2026-11-03-migration-consolidation.md#data-and-behavioral-comparison)
now allows a clock difference only for the named fresh singleton rows and only
after both satisfy the independent seed and install-window checks. Each report
retains the raw snapshot digests. Comparisons of existing databases remain exact.
The install window spans `up` and `status`. Per-migration clock bounds and
independent schema/data assertions at each retained boundary remain pending.

On 2026-11-10, `verify-migrations fresh` applied the current 587-step executable
plan to each of two empty PostgreSQL 16.13 fixtures. Both replays satisfied the
ordered step and final journal checks. The report retained distinct raw digests,
listed eight validated clock differences and found no blocking difference. All
28 scoped library and verifier tests passed, including ordinary data drift, an
out-of-window bootstrap value and database and role name collisions. Both
fixture databases and roles were removed.

On 2026-11-23, `xtask migrations verify-fresh` compared two source snapshots
on PostgreSQL 16.13. The reference was local `main` at `266e7e6d56`, with 598
migrations. The disposable candidate folded Blog's list and retention indexes
into its initial schema, skipped creation of the redundant preview-token index,
and retained the separate translation-width migration and rollback. Its 597-step
plan differed only by the removal of `m20261030_000002_add_blog_list_indexes`.
Both journals matched their executable plans. The report found no blocking schema
or data differences and recorded eight validated bootstrap clock differences.

The seeded Blog replay used the same two pinned executables and compared copies
of one PostgreSQL 16.13 predecessor after 547 migrations, immediately before the
shared Blog creation identity. Both runs seeded posts, a translation, a preview
token and a comment-delivery record after Blog creation. Each subsequent
migration ran in its own process, and its confirmation and exact journal
prefix were checked. Five injected journal failures covered Blog creation and
translation widening on both paths and index creation on the reference path.
Retries kept the expected widths, indexes and fixture rows. Both schemas enforced
preview-token uniqueness after removing the redundant index. Strict comparisons
found no differences at the end of the Blog range or after all 598 reference and
597 candidate migrations. Each final journal matched its own executable plan.
The predecessor and both databases were removed.

The source-coverage check added in `53405e9228` reused the retained reference
build at `266e7e6d563a2b5d1367211bd4c7245f5872562f`. Discovery matched all
3009 paths from Cargo's dependency list to that Git tree. It found a source
entrypoint and verified local links for all 598 executable migrations,
including three with explicit `MigrationName` implementations. The generated
declarations passed `bind-inventory` with all 598 source fingerprints attached
in executable order. This used the existing PostgreSQL report and the saved
Cargo dependency list. It establishes local source coverage for that build.
The saved report predates the compiler-source observation field. The
binding regression used a separate copy with the recorded observation and
rejected changed dep-info bytes, a changed path root and a declaration for
an unobserved file. The original pilot files were preserved.

The original pilot files are retained locally under
`target/migration-pilot-20261122`. This record identifies the verified files
without tracking the scratch trees, build outputs or raw database snapshots.

| Retained file | SHA-256 |
|---|---|
| Fresh `plan.json` | `aa5fd76d4491f9257493713fbd716d42737641fcdc807bfc8fd167956fc40cd1` |
| Fresh `report.json` | `dc2a166cc5bacd37220db006c6e06a6f37bcaec2d1a89a0506777c7604efe3a2` |
| Fresh `database-report.json` | `77c6a7c881f34c6aca47de2f8ca8008f7d94f7944a3a3de1324b5173e32ebe5d` |
| `reference-plan.json` | `0528743c2915f0a71ba7ba72b1319ba61a507649baf68ccf31daf6a902ea76b8` |
| Reference `export-migration-plan.d` | `949322d58ce3ba8a9fade0c52f5c4778d5e6d55e3081b5aea1f099055ff4030d` |
| Seeded `checkpoints.json` | `8d54604deba94ecdf77d93064bf26fe1eac8bb8f5c9df5467445e51fa9988d85` |
| Seeded `full-composition-comparison.json` | `aee8a79ff355d344400d3c900c0a1262e22021b4b3c31b2ae55f7bdd5777d958` |

For another review of the same inputs, check the retained file digests and
reuse the recorded results. A changed source, build, fixture, database
environment or release boundary requires evidence for the changed inputs.
Durable storage for the full source and report set remains part of release
admission.

`registry-inputs` reused the same reference build's dependency resolution
and committed lockfile. All 1011 selected crates.io archives matched their
locked checksums. All 47,106 packaged file paths and byte digests matched
the supplied extracted cache. The result is retained under the pilot root
as `registry-cache-reference.json` with SHA-256
`2207bc20f110901007d2027f77f565e241f30bd42499881dc57fa8702bd3e0b8`.
It establishes the observed cache contents for this package selection.
The build-time check was exercised on an offline Cargo fixture using
the resolved `hex` 0.4.3 package. Cargo's manifest path matched the inspected
cache, and both endpoint observations were equal. The regression also
rejects a different manifest path or modified extracted source bytes.
For generated outputs, a separate offline Cargo fixture compiled a Rust
module loaded through `include!` from `OUT_DIR`. The recorded SHA-256
matched the file. The verifier rejected an output directory outside the
build target and edits, additions and deletions after capture. The earlier
PostgreSQL pilot predates the generated output record.
The fixture also verified dep-info for both binaries on Windows with spaces
in the target path. Both dependency lists matched the included Rust file.
The unused output was captured and absent from the dependency lists. A swapped
dep-info file and a missing generated dependency failed verification.

A separate PostgreSQL 16.13 test on the current composition rolled back the
Blog index migration, then confirmed that over-width translation data blocks
column narrowing without altering the row or journal. With compatible title
and description lengths, rollback reduced their limits while preserving the
32-character locale contract. Reapplying both steps restored the indexes and
full column widths. [Blog rollback test](../../crates/utils/rustok-migrations/tests/postgres_zero_migration_smoke.rs)

The installer schema smoke on PostgreSQL 16.13 applied the five bootstrap
migrations on an empty database and retried them before completing the full
composition. It verified contention with the stepwise runner, transactional
rollback and retry of attempt fencing with an existing journal, and rejection
of an unknown journal identity. The fixture database was removed.

On 2026-11-23, the installer adapter loaded an Ed25519-signed fixture receipt
on PostgreSQL 16.13. It rejected a tampered receipt, imported the original after
bootstrap, and resolved the same release before and after applying the complete
schema. A repeat import reused the original record. The fixture database was
removed. [Signed base-distribution test](../../crates/utils/rustok-installer-persistence/tests/postgres_base_distribution_import.rs)

The Tenant locale backfill ran on PostgreSQL 16.13 at `main@c555cc3493`.
The fixture seeded a tenant in the schema immediately before
`m20260726_000001_enforce_tenant_locale_policy`. A deliberately failing
post-step assertion stopped execution before the next migration. After restart,
the runner completed the remaining composition and the tenant still had one
enabled default locale at policy revision zero. The fixture database was
removed. [Backfill recovery test](../../crates/utils/rustok-migrations/tests/postgres_zero_migration_smoke.rs)

The integration of upstream `f94c88fc06` repeated the recovery checks on
PostgreSQL 16.13. The stepwise suite covered transaction failure, cancellation,
connection and process loss, and restart. The composed smoke applied all 598
migrations, rolled back `m20261121_000001_add_sys_events_tenant_projection`,
and reapplied it. The installer schema and signed import checks passed on
separate fixtures. The database observer also detected changes to schema,
data, privileges, and index enforcement. The fixture databases and roles
were removed.

The seeded Modules phase test used the composition from `main@59b2c3f1cf`
on PostgreSQL 16.13. It cloned a single quiescent predecessor seeded with
three declared fixtures. One copy
applied the five migrations from
`m20261027_000081_transition_intervention_required_state` through
`m20261027_000085_artifact_ui_resources` in one session. The other reopened
the same phase after each committed step. The owner assertions passed at
their own boundaries, and both final observations had identical digests and
matching journal identities. The predecessor and both copies were removed.
[Seeded Modules phase test](../../crates/utils/rustok-migrations/tests/postgres_zero_migration_smoke.rs)

The same PostgreSQL 16.13 test at `main@2a6647ebfa` rejected the journal
insert for `m20261027_000085_artifact_ui_resources` after the migration
created and populated the projection. The failed runner refused a retry.
The transaction rolled back the projection and its index, and the journal
retained the exact pre-step identities. The full pre-step and post-failure
observations had identical digests. After the injected constraint was removed,
a new session completed the phase and matched the uninterrupted database.

Gate 3 still needs replay from an authenticated stable release baseline.
Gates 2, 4 and 5 remain pending.
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
