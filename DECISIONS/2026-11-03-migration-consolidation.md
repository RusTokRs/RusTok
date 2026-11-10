# Release-bound migration consolidation

- Date: 2026-11-03
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform-architecture, xtask, rustok-migrations, domain migration owners, release tooling
- Extends: 2026-05-18-control-plane-lifecycle-and-migration-contracts, 2026-07-02-axum-runtime-and-ops-cli-boundary, 2026-08-06-module-release-rollback-safety
- Supersedes: None
- Superseded by: None

## Context

The agreed product policy freezes migrations at every stable release, including
patch releases. Unpublished migrations may be consolidated between releases.
Prereleases run on disposable development/test databases. Persisted state that
must survive has an explicit migration requirement regardless of its label.

The existing source scanner recognizes a subset of Rust/SeaQuery/SQL syntax.
It cannot establish schema or data equivalence. Current CI compares migration
names from PR base and head, then exercises fresh/incremental PostgreSQL,
one-step rollback and base-to-head upgrade. It neither seals migration contents
at a stable release nor compares original and consolidated results.

This decision defines the missing verification and publication mechanism. Its
status does not authorize a bypass of the current append-only gate. The
[guide](../docs/guides/safe-database-migrations-and-squashing.md) owns operational
instructions, current limitations and implementation sequencing.

## Decision

Extend existing execution, verification and release tooling in Rust. Prepare
candidate changes in the current working branch and compare them with immutable
reference source snapshots on real PostgreSQL. Admit replacement of draft
migrations only when every required check passes. Any edit to the candidate
invalidates previous evidence.

### Rust tooling boundary

`cargo xtask migrations` is the developer and CI entrypoint. `xtask`
owns argument validation, trusted source/reference selection, source snapshots,
candidate previews, process orchestration and report delivery. Reference builds
use disposable directories and do not create branches.

`rustok-migrations` owns actual catalog observation, canonical migration
execution, typed schema/data comparison and the reusable fixture execution
contract. Expose those capabilities through scoped owner tools invoked as
processes. Avoid adding its unconditional domain dependency tree to `xtask`.
`pg_dump` and `pg_restore` may be invoked as pinned PostgreSQL tools from Rust.
Domain migration contents and assertions remain owner-owned.

Scoped domain verification consumes the creation migrations for the shared
tenant/user/session/authorization identity schema through
`rustok_migrations::identity::migrations()`, available without `composition`.
The full `Migrator` composes that same source; fixtures must not copy foundation
table definitions or enable unrelated modules just to satisfy foreign keys.
This bounded schema source is not a second production migrator or permission to
omit a selected module's prerequisite migrations.

The same Rust workflow runs locally and in CI. Once the verified Rust workflow
is operational, replace the current JavaScript source-squash entrypoint and
update its callers in the same cutover. Existing release publication policy
retains its owner. Source snapshots, fresh-install execution and PostgreSQL
observation are implemented. Full release/range/recovery admission remains
unfinished and blocked by the gates in the canonical guide.

### Release boundary and provenance

Each stable distribution release publishes an attested migration manifest bound
to its exact signed source commit and release artifacts. Reuse the existing
signed-tag, immutable-release, checksum and attestation chain. The manifest is
derived from executable composition, never maintained as a second handwritten
registry of migration truth.

The manifest records owner-qualified migration identities, actual execution
order, dependency edges, safety/phase metadata and hashes of migration source
inputs. Hash nested Rust modules, included SQL/assets and declared helper
dependencies as well as entrypoint files. Record the resolved lockfile,
toolchain, build scripts/generated inputs, features, composition, PostgreSQL
version/extensions and verifier version in the enclosing build identity.

The current executable inventory supplies the declaration portion through
`Migrator::inventory()`: one module source list yields both executable objects
and ordered export entries, with ownership derived from the module contract.
Typed registrations derive that slug from the concrete module and retain only
its narrow `MigrationSource` port, avoiding a full module lifecycle vtable in
the operational inventory.
Missing safety metadata and undeclared transaction policy remain explicit
`null`. These declarations do not establish source-input coverage or observed
safety. Trusted immutable hashes, transitive build provenance and stable seals
remain required before this inventory can participate in release admission.

The local `xtask migrations verify-fresh` report now binds that export and the
runner/exporter digests to the enclosing path/mode/blob snapshot, lockfile,
repository Cargo/toolchain configuration, native filtered dependency resolution,
tool versions and explicit build arguments. Resolution is compared before and
after the build. Normal/build dependency identities and selected library,
proc-macro, build-script and runner targets use snapshot-relative or
package-relative paths. Workspace feature unification remains an overestimate,
not an exact compilation-unit graph. External bytes, generated inputs,
build-script/proc-macro effects, ambient native/toolchain inputs and individual
migration attribution remain unverified. This partial local evidence does not
establish the complete provenance or authority required below.

Trusted inventory must establish the complete transitive execution-input
coverage, including shared helpers and generated input provenance. Declarations
are checked against resolved build/source inputs. An unresolved input or mutable
ambient dependency produces incomplete evidence. Candidate-produced JSON alone
cannot establish source coverage. Consolidation runs use deterministic recorded
inputs and isolated external-effect adapters.

Released migration bodies, migration-local helper inputs, identities and
declared execution constraints are immutable. Dependency/toolchain upgrades
must also prove that replay of the released prefix retains its sealed schema
and data contract. A names-only check or an unchanged entrypoint hash cannot
establish that property. New migration helpers should be migration-local and
avoid mutable domain runtime behavior.

Resolve the protected boundary through trusted release tooling. A PR cannot
choose an older tag, replace an attested manifest, declare itself prerelease
or select an empty boundary to reduce protection. Include all previously
published stable distribution and independently published native module seals.
Their protected histories form a union. Ending support reduces the required
upgrade matrix and never unlocks published migrations. A platform patch version
does not make an independently released module's migrations editable. Trusted
validation derives source-input hashes from immutable Git trees and verifies
candidate exports against that inventory.

Before the first stable release, an explicitly recorded bootstrap state permits
an empty released prefix. Missing release evidence after publication blocks
consolidation. Trusted integration and release admission serialize their final
decisions against one monotonic seal generation, or use equivalent atomic
conditional acceptance. A check followed by an unfenced merge/publish is invalid.
Reserve the stable migration identity before the first externally consumable
stable artifact, including a container image. Failure after exposure retains
the reservation and its frozen history. New seals are monotonic.

### Reference, candidate and allowed replacement

Capture three immutable inputs for each run. `B` is the released predecessor,
`O` is the original draft source, and `C` is the proposed consolidated source.
Trusted admission selects O from the protected pre-consolidation source, normally
the PR base or a separately reviewed preparatory commit containing the complete
original tail. A caller-selected historical or fabricated reference is invalid.
Pin full source/build identities and fixture digests. Retain the reference source
archive and report durably so removed draft code remains reproducible. Before
admitting removal, store the source/build inputs and reports by content digest,
verify retrieval and record those identities in trusted admission. Expiring CI
artifacts alone are insufficient.

Describe a complete mapping from removed draft migrations to surviving candidate
migrations. Each removed migration has an owner-reviewed disposition covering
its DDL, initial data, backfills, invariants and execution phases. A disappearing
temporary object needs an explicit explanation. Changed domain behavior is
reviewed as a separate contract change rather than hidden by consolidation.

Retain released identities, content and dependency contracts. Validate the whole
composed graph. A chain `A1 -> B1 -> A2` cannot become one migration for owner A
without introducing a cycle. Preserve installer bootstrap cuts, external
dependency cuts, activation phases, checkpoints, nontransactional operations and
recovery boundaries. Several migrations per module/release are valid.

Every removed and candidate migration requires reviewed safety, phase, locking,
backfill and recovery constraints. `migration_safety_metadata()` can currently
return an empty vector. Missing metadata means incomplete evidence and must
never imply additive safety or unrestricted execution.

### Execution matrix

For each affected supported composition and PostgreSQL environment, execute the
following comparisons through owner-owned production migration entrypoints.

| Run | Starting state | Reference path | Candidate path | Required result |
|---|---|---|---|---|
| Fresh install | Two empty databases | Full O composition | Full C composition | Equivalent catalog and required initial data, independent owner assertions |
| Released replay | Empty database | Sealed B artifact/source | C implementation of the B prefix | Sealed catalog, initial-data and migration-identity contracts preserved |
| Upgrade | Two restored copies of one seeded B snapshot | Original O tail | Consolidated C tail | Equivalent catalog and canonical data, independent expected results |
| Retry and failure | Identical B fixtures | Original recovery contract | Candidate interruption and restart at retained boundaries | No silent loss, duplicate effects or false completion |
| Installer | Empty installation storage | Bootstrap/import/remaining schema in O | The same lifecycle in C | Bootstrap availability and import ordering preserved |

Repeat the upgrade check for every supported predecessor, or prove the supported
multi-release upgrade path through its intermediate stable releases. Missing
profile, predecessor, fixture or environment coverage blocks the corresponding
claim. The current all-module migrator is the first coverage target. Profile
selection requires real composition support before it can be certified.

For the initial baseline B is empty. Historical draft-only intermediate data is
disposable only under the agreed data policy. Any retained beta/development data
requires a separate deterministic transfer and restore rehearsal.

### Catalog comparison

`rustok-migrations` observes actual PostgreSQL catalogs into a deterministic,
typed snapshot. The verifier compares semantic identities and definitions using
the same pinned server version, extensions, locale and role setup on both sides.
Retain raw schema dumps for diagnosis and reproduction.

The inventory includes schemas, tables/partitions, column order and types with
typmods/collations, nullability, defaults, generated/identity definitions,
sequences, PK/unique/FK/check/exclusion constraints with validation/deferral and
referential actions, indexes with expressions/predicates/include/opclasses,
views/materialized views, functions with security/configuration attributes,
triggers and enabled state, RLS flags/policies, ownership, ACLs and default
privileges. Extension versions and extension-owned objects are accounted for
explicitly. An unsupported object or unreadable definition is a verification
failure.

Normalize internal OIDs, database-instance identifiers and presentation ordering
only. Preserve observable names, column order, function/trigger bodies and
security semantics. Fixed role mappings may translate scratch role identities
without discarding their privileges. Generic regex removal of differences and
broad object ignore lists are forbidden.

Compare the `seaql_migrations` schema normally. Check its rows separately against
each path's exact expected identities and completion state. Different draft
history is an expected consequence of consolidation. Unknown or missing records
and pending migrations are failures. Never rewrite a production migration
journal to make replacement appear compatible.

`pg_dump --schema-only` is a supporting artifact. Data and sequence values need
separate checks. Avoid treating `--no-owner` or `--no-acl` output as security
equivalence evidence. PostgreSQL documents these scope distinctions in its
[pg_dump reference](https://www.postgresql.org/docs/16/app-pgdump.html).

### Data and behavioral comparison

Seed B once and restore the same consistent snapshot into both upgrade branches.
Use synthetic, deterministic, owner-defined fixtures, including multiple tenants,
locales/channels, empty/boundary values and realistic preexisting relationships.

Inventory every persistent application table and compare typed row contents,
including tables outside the declared owner range. Key by stable identity or
compare multisets with duplicate multiplicity when no stable key exists. Any
journal/derived/nondeterministic exception needs an explicit owner-reviewed
disposition. Do not infer the compared set solely from the range mapping, as a
trigger or raw SQL may mutate an undeclared owner. Row counts alone are
insufficient. Preserve exact decimal, JSON, timestamp,
binary and NULL semantics. Explicitly cover IDs, counters, revisions, outbox
records, receipts and queued work. Check sequence state and next-insert safety.
Do not require gapless sequences, as PostgreSQL permits gaps after aborted
operations. See the [sequence reference](https://www.postgresql.org/docs/16/functions-sequence.html).

Fixtures assert independently expected target results on both paths. Matching a
bug in O is insufficient. A proposed fix to O's behavior requires a separately
reviewed target expectation and an explicit, narrowly identified difference.
Deterministic IDs/clocks are preferred. Any nondeterministic-field equivalence
must be owner-defined, field-specific and incapable of hiding lost rows or
changed references.

For stepwise replay from an empty PostgreSQL database, `updated_at` records the
time of bootstrap for the following singleton rows in `public`:

- Channel's `channel_resolution_invalidation_state` and RBAC's
  `rbac_invalidation_state` start at generation zero for their respective scopes.
- Flex's `flex_field_definition_cache_generation` starts at generation zero for
  its singleton key.
- Modules' `module_artifact_node_reconciliation_state`,
  `module_static_distribution_state`, `module_static_distribution_release_state`
  and `module_static_distribution_rollout_state` start at revision zero with no
  build, release, rollout or reconciliation references.
- Modules' `module_security_epoch` starts at epoch one with the initial
  bootstrap reason. Its timestamp identifies when that epoch was initialized.

The fresh verifier may accept different timestamps for those exact fields when
both observations contain one row with the expected key, exact column layout
and types, and all expected non-clock values. Each timestamp must be finite and
fall within that side's PostgreSQL-measured install window. Any invalid observed
bootstrap row fails even when both sides produced the same row. The report
retains both raw snapshot digests and identifies each accepted clock difference.
Schema, privileges, enforcement, sequences and all other data compare exactly.
The disposition applies only to the verifier's empty-fixture replay. Comparisons
of existing databases, upgrade data and release admission retain their own
requirements. It does not authorize any migration edits or data rewrites.

Every removed migration has a data disposition. Extend the current backfill
fixture contract to cover ranges and multiple cases. Setups that require an
intermediate draft schema must be redesigned against B or tested as an explicit
retained intermediate boundary. Copying their old setup SQL to B is invalid.

Run owner CRUD, invalid-data rejection, uniqueness, tenant and relevant runtime
consumer assertions on C. RLS tests use the intended application roles, including
a non-owner role without bypass authority. Owners commonly bypass RLS, as
described in the [PostgreSQL policy documentation](https://www.postgresql.org/docs/16/ddl-rowsecurity.html).

### Admission and evidence

Keep the base-owned infrastructure approval and untrusted-code isolation model.
Read-only PR checks produce a content-bound report. Integration checks the exact
merge-result identity and current seal generation under the admission fence.
Squash/rebase results require fresh final evidence for their resulting source
and build identity. Draft replacement is allowed only for the declared mapping
with complete evidence. Other changes
continue through the normal migration compatibility policy.

Before publication, trusted release tooling verifies exact-release-commit
evidence and emits the next immutable manifest through the existing release
asset/attestation pipeline. A report records input commits/build digests,
protected boundaries, range mapping, environments, catalog/data digests,
fixtures/assertions, recovery results and explicit coverage. No fixture contents,
credentials or production row dumps belong in public reports.

Required-check configuration and publication dependencies must enforce the actual
gates. A skipped, missing or renamed check cannot satisfy admission. Verify the
repository settings as well as workflow source wiring.

A fixture mismatch, incomplete inventory, timeout or uncertain executor outcome
prevents success. Evidence collected for another commit or composition is stale.
Finite fixtures certify the exercised contracts and environments. They do not
establish equivalence for arbitrary Rust/SQL programs or every possible dataset.

## Sources of truth and ownership

- Domain `MigrationSource` implementations own schema/data mutations, dependency
  descriptors and safety metadata.
- `rustok-migrations` owns executable composition, observed catalog snapshots and
  execution results. Reuse its PostgreSQL fixture support.
- `xtask` owns the Rust developer/CI workflow and calls scoped owner tools.
- Release tooling owns stable publication identity and immutable manifests.
- Verification scripts orchestrate immutable inputs, comparisons and reports.
- `rustok-modules` retains deployment authorization, fences and recovery policy
  under the existing release-safety ADR. A consolidation report grants no live
  mutation authority.

## Invariants

The protected stable history survives intact. Every removed draft migration is
accounted for. Both fresh install and supported predecessor upgrade are checked.
All compared paths use the same production composition and environment. Unknown
objects, missing coverage and weaker execution/recovery boundaries block admission.

## Non-goals

Runtime migration execution remains under the existing installer/operations and
release lifecycle contracts. Automatic consolidation concerns unpublished source
only. The source scanner remains a bounded diagnostic. SQLite checks retain their
portable-test scope.

## Data, transaction, and concurrency boundary

Verification runs in isolated disposable databases and immutable checkouts with
bounded roles and timeouts. Clone only quiescent fixture snapshots. Fence fixture
writers while observing schema/data. Retain each path's transaction and
nontransactional DDL boundaries, and revalidate inputs before accepting evidence.
The stepwise verification runner owns one PostgreSQL session for both its
advisory lock and migration execution. It must not reconnect after session loss.
Each committed step must leave exactly the expected journal prefix before any
following step or independent boundary assertion runs. An interrupted runner
must be discarded and its journal reconciled through a new session. Until an
owner-proven nontransactional recovery contract is wired, this runner rejects
nontransactional steps before making database changes; it must never silently
wrap them in transactions or retry their possibly committed partial effects.
Rollback rehearsal covers only owner-declared reversible boundaries. Irreversible
paths need tested restore/forward recovery and preserve maintenance authorization.

## Context dimensions

Bind evidence to owner/module versions, composition/features, predecessor,
database/version/extensions, locale/collation and role policy. Fixtures cover
tenant, channel, locale and authorization dimensions that affect the changed
owner. Distribution and module release identities remain distinct.

## Events and projections

Preserve canonical outbox/receipt/work state and owner event semantics. Derived
indexes/caches may use an owner-defined deterministic rebuild followed by parity
assertions. External effects execute only through isolated test adapters and
retain idempotency/recovery checks.

## Failure semantics

Return a failed or incomplete report for missing evidence, cleanup failure,
interrupted execution or uncertain outcome. Retain redacted diagnosis and exact
input identities. A retry recreates its scratch state or resumes through the
owner's proven recovery path. No success default or implicit data discard is
permitted.

## Migration and cutover

The ordered implementation gates live in the canonical guide. First restore
trustworthy current checks, then implement release sealing, catalog observation,
data/recovery comparisons and finally controlled CI admission. Keep the existing
append-only behavior until the replacement admission path is operational.

## Alternatives considered

Regex-based source equivalence cannot model arbitrary helpers or SQL execution.
Generating a baseline from entities loses owner-defined database objects and data
effects. A schema dump alone omits upgrade behavior and runtime invariants.
Concatenating files retains temporary work and does not establish safe phase
boundaries. A new migration engine would duplicate the existing execution owner.

## Verification

Adversarial cases must reject deleted constraints/triggers/RLS, wrong types,
lost backfills/initial data, cross-tenant reassignment, changed published helpers,
weakened safety phases, cycles, missing bootstrap checkpoints, stale evidence,
empty scans and unavailable release seals. Test cleanup and interruption too.

Run scoped Rust tests for the execution/observation owner and Node tests for
policy/admission. Run PostgreSQL comparisons for each claimed composition and
predecessor. Preserve original source archives and machine-readable reports.
No database equivalence or production-readiness claim is made by this ADR.

## Consequences

The design adds database executions and durable evidence to release preparation.
It permits removal of unpublished intermediate work while preserving supported
upgrade and recovery contracts. It deliberately optimizes for verifiable behavior
and explicit coverage rather than a fixed number of migration files.
