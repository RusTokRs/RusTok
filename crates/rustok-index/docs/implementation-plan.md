# Implementation plan for `rustok-index`

## Mission

`rustok-index` is the platform-owned cross-module relational index and query
engine. Source modules publish generic schemas, records, mutations, and links;
Index materializes them into optimized relational storage and executes filtering,
projection, sorting, counting, and pagination without runtime fan-out to source
modules.

`rustok-index` is not a search engine. Ranking, relevance, typo tolerance,
synonyms, autocomplete, search UX, and external search-engine connectors remain
owned by `rustok-search`.

## Scope

This plan covers the generic schema and link registry, validated records and
mutations, PostgreSQL persistence, query planning/execution, incremental
ingestion, rebuild/reconciliation, operator controls, and the first owner-published
vertical slices. It excludes text relevance, ranking, autocomplete, external
search engines, source-table reads, and source-domain semantics in Index core.

Detailed completed-work evidence belongs in accepted ADRs, committed evidence
packets, and Git history. This document remains the live roadmap and verification
contract.

## Rewrite policy

The project is in early development. **Backward compatibility with the rejected
implementation is not a goal.** Existing code, migrations, APIs, ports,
adapters, tests, fixtures, evidence, and documentation may be deleted or
replaced whenever they conflict with the target architecture.

## Update rules

1. Prefer a clean replacement over a compatibility layer.
2. Do not preserve placeholder APIs or tests that encode rejected architecture.
3. Index core must never query source-module tables directly.
4. Product, Content, Flex, Pricing, Inventory, and other source semantics must
   not be hard-coded in the generic engine.
5. Every completed task is checked off here in the same change.
6. Public boundary changes update local docs, the module manifest, central
   registry, verification scripts, and architecture decisions.
7. A milestone is complete only when its acceptance criteria are satisfied.
8. Benchmark scaffolding is not production persistence and must not leak into
   `rustok-index` migrations or runtime composition.
9. Partition cutover remains forbidden until retained PostgreSQL shadow evidence
   satisfies an explicit admission policy.
10. Source modules publish schema contracts through Index-owned APIs; they never
    write `index_schemas` or other Index-owned tables directly.

The repository owner performs test and benchmark execution during this rewrite.
Commits and pull requests record which checks and evidence runs were not executed.

## Current state

- Rewrite status: `in_progress`
- Current milestone: `M3 - PostgreSQL storage engine`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: `complete`
- M1 domain/application core: `complete`
- M2 storage benchmark: `complete`
- M2 storage decision: `JSONB accepted; rejected prototypes removed`
- M3 storage-schema foundation: `complete`
- M3 atomic mutation persistence: `complete`
- M3 persisted source schema registration: `complete`
- M3 schema-application leases: `complete`
- M3 secondary-index lifecycle: `complete`
- M3 partition admission and shadow planning: `complete`
- M3 partition evidence packet tooling: `complete`
- M3 partition evidence capture/assembly: `complete`
- M3 partition baseline/shadow snapshot runner: `complete`
- Production persistence: tenant-scoped schema registration, mutation writes,
  schema coordination, secondary-index lifecycle, fail-closed partition admission,
  snapshot capture, evidence assembly, and evidence validation implemented; the
  real query/mutation/maintenance/cutover partition evidence, query adapter, and
  production partition lifecycle are not yet implemented

The active production crate contains the generic domain/application core, the M3
production migrations, an Index-owned tenant schema registration store, an
Index-owned transactional mutation adapter, a durable schema-application lease
store, a schema-derived secondary-index manager, and a measured
partition-admission contract that emits shadow bootstrap plans only. The repository
also contains immutable partition-manifest preparation, owner-operated
baseline/shadow snapshot capture, exact-byte raw artifact assembly, and measured
packet validation tooling. Query adapters, retained query/mutation/maintenance/
cutover partition evidence, production copy/constraint/index attachment,
replay/cutover, batch ingestion, source-provider runtime registry, and PostgreSQL
Testcontainers evidence remain open. Benchmark DDL and generated evidence stay
under `ops/benches`, outside the production module.

## Ownership

`rustok-index` owns schema/link registration, generic records and mutations,
ingestion, inbox deduplication, relational storage, query validation/planning,
SQL compilation, filtering, projection, sorting, counting, pagination, rebuild,
checkpointing, reconciliation, drift repair, distributed coordination, and
operator diagnostics.

Source modules own normalized domain data, schema declarations, conversion to
generic Index records/mutations, paginated rebuild scan/load adapters, and
source ordering/version information. They call Index-owned schema and mutation
APIs and never write Index storage directly.

`rustok-search` owns text relevance, ranking, typo tolerance, synonyms,
autocomplete, search UX, external search engines, and search-specific result
enrichment through stable Index contracts.

## Target architecture

```text
source modules
    -> owner-published IndexSchema
    -> PostgresSchemaRegistrationStore
    -> IndexSource / IndexMutation
    -> ingestion and rebuild engines
    -> PostgreSQL index storage
    -> schema/link registry and query planner
    -> SQL compiler
    -> IndexQueryPort
    -> server, storefront, admin, and rustok-search
```

```text
crates/rustok-index/src/
  domain/
  application/
  migrations/
  infrastructure/
    postgres/
      mutation_store.rs
      schema_registration.rs
      schema_lease.rs
      secondary_index.rs
      partition_admission.rs
    events/
    telemetry.rs
  api/
    query.rs
    admin.rs

ops/benches/src/index_storage/
  config.rs
  runner.rs
  mutation_runner.rs
  maintenance_runner.rs
  partition_snapshot.rs
  sql/
```

## Library decisions

Use existing workspace libraries where possible:

- `sea-orm` and SeaQuery for PostgreSQL connections, transactions, migrations,
  execution, and dynamic SQL;
- `tokio` and `futures-util` for bounded async work;
- `serde` and `postcard` for DTO/cursor serialization;
- `thiserror` for typed errors;
- `tracing`, `rustok-telemetry`, and `prometheus` for observability;
- `proptest` and `criterion` for invariants and benchmarks;
- `moka` only for immutable schema/compiled-plan local caching.

Selected additions:

- `petgraph` for deterministic schema/link graph traversal;
- `icu_locale` with compiled ICU4X data for UTS #35/CLDR locale alias
  canonicalization;
- `sha2` for stable schema fingerprints, cursor checksums, secondary-index
  definitions, partition shadow-plan identities, and retained evidence digests;
- `postcard` plus URL-safe Base64 for versioned keyset cursors.

Add when required:

- `tokio-util` for cancellation/task tracking;
- `backon` for classified retries;
- `testcontainers-modules` with PostgreSQL;
- `insta` for plan/SQL/schema snapshots.

Forbidden in Index core:

- ranking/search-engine libraries;
- a second ORM/database stack;
- custom graph, locale, retry, or executor implementations;
- collecting all rebuild IDs in memory;
- source-table reads;
- source-domain crate dependencies;
- unvalidated JSON-only public queries;
- source-owned direct writes to Index tables;
- destructive partition cutover without retained evidence and rollback proof.

## Milestones

### M0 - Hard reset and architecture lock

- [x] Replace the implementation plan with the Index Engine roadmap.
- [x] Record rewrite policy and target ownership in an ADR.
- [x] Reset local FBA readiness to `in_progress`.
- [x] Remove legacy v1 ports, adapters, source-specific indexers, projections,
      migrations, runtime configuration, scheduler, server composition, and direct
      source-table reads.
- [x] Remove source-domain dependencies and add guards preventing their return.
- [x] Synchronize local and central module documentation.

M0 is complete. No compatibility contract exists for deleted Index v1 behavior.

### M1 - Domain core and schema registry

- [x] Add bounded identifiers, canonical locales, schema identities, and versions.
- [x] Add `IndexValue`, `IndexRecord`, `IndexMutation`, `IndexSchema`, and link
      metadata.
- [x] Add stable order-independent SHA-256 schema fingerprints.
- [x] Add atomic versioned schema registration and deterministic link-path
      resolution.
- [x] Validate records, mutations, query paths, operators, types, cardinality,
      tenant/locale scope, and complexity bounds.
- [x] Add versioned checksummed query-scoped keyset cursors.
- [x] Add a test-only reference mutation/query engine and property invariants.

M1 is complete. Product and SalesChannel are representable by ordinary generic
schemas and links without Product-specific Index code.

### M2 - PostgreSQL storage benchmark

- [x] Define the benchmark contract and keep candidate DDL outside production
      migrations.
- [x] Add deterministic `smoke`, `100k`, and `1m` dataset presets.
- [x] Prototype JSONB entity rows plus typed expression/GIN indexes.
- [x] Compare JSONB, typed EAV, and specialized hot projection using equal result
      digests, cardinality, planner/session metadata, WAL, buffers, relation size,
      churn, and VACUUM evidence.
- [x] Run and archive replacement 100k Product-locale row read, mutation, and
      maintenance evidence from the accepted same-commit packet.
- [x] Run and archive replacement 1m Product-locale row read, mutation, and
      maintenance evidence from the same accepted commit.
- [x] Compare warm/cold buffers, planner stability, execution latency, ingestion
      throughput, relation size, WAL, dead tuples, vacuum behavior, and operational
      complexity.
- [x] Record the selected model and rejected alternatives in an ADR.
- [x] Delete benchmark prototypes that are not selected.

M2 is complete. The remaining JSONB benchmark path is a selected-layout regression
harness; it is not production persistence and does not reopen the accepted storage
decision. Partitioning was not measured by M2, so the canonical tables remain
unpartitioned until a separate shadow packet passes admission.

### M3 - PostgreSQL storage engine

- [x] Add canonical schema/entity/link/inbox/job/checkpoint/consistency migrations.
- [x] Add tenant/schema/entity/locale keys and source-version guards.
- [x] Add atomic entity/link upsert and delete transactions.
- [x] Add tenant-scoped persisted source schema registration with exact idempotency,
      monotonic versions, retired-state protection, and source-neutral ownership.
- [x] Add locking/leases for schema application.
- [x] Add secondary-index planning and lifecycle management.
- [x] Add fail-closed partition admission and deterministic tenant-hash shadow
      planning.
- [x] Add immutable partition evidence manifest, measured packet validator, and
      owner-operated runbook.
- [x] Add exact-byte raw-artifact capture assembly with bundle confinement and
      no-clobber packet publication.
- [x] Add owner-operated PostgreSQL baseline/shadow snapshot capture.
- [ ] Execute retained PostgreSQL query, mutation, maintenance, and cutover evidence.
- [ ] Add partition copy, constraint/index attachment, replay/dual-write, cutover,
      rollback, and durable global operation ownership.
- [ ] Add PostgreSQL Testcontainers fixtures.
- [ ] Cover migration-from-zero, schema registration concurrency, stale mutation,
      redelivery, rollback, and tenant/locale isolation in PostgreSQL.

The first M3 slice registers the seven module-owned tables. Their keys lead with
tenant identity, preserve the complete schema/entity/locale shape, bind entities
and links to exact non-negative `DECIMAL(20,0)` source versions, and retain schema
fingerprints in entity foreign keys.

The second M3 slice publishes `PostgresMutationStore`. Every delivery is validated
through `SchemaRegistry`, bound to a SHA-256 payload identity, claimed through the
composite inbox key, serialized by a transaction-scoped entity advisory lock, and
applied atomically. Exact redelivery is a duplicate; stale versions are terminally
ignored; deletes write payload-free tombstones.

The third M3 slice publishes `PostgresSchemaLeaseStore`. It serializes exact
tenant/module/entity/schema-version application with a transaction-scoped advisory
lock, verifies the persisted active schema and fingerprint, records durable
`schema_apply` work in `index_jobs`, and returns `Busy` or `AlreadyApplied` when
appropriate. Expired work is reclaimed with incremented attempt fencing. Heartbeat,
success, and failure require the exact job, worker, attempt, running state, and an
unexpired lease.

The fourth M3 slice publishes `SecondaryIndexPlan` and
`PostgresSecondaryIndexManager`. Plans derive deterministic tenant- and
schema-fingerprint-bound indexes from filterable/sortable fields. Scalar fields use
partial typed B-tree expressions ordered by locale/value/entity identity;
filterable `many` fields use field-local JSONB containment GIN. Stable names bind
the complete definition hash. `secondary_index` jobs coordinate ensure, concurrent
reindex, and concurrent retirement with advisory locking, expiry reclaim,
heartbeats, attempt fencing, persisted schema validation, owner comments, and
PostgreSQL readiness/validity inspection. Expressions follow the production tagged
`IndexValue` payload through each field's `value` member. SQLite remains
contract-test-only; PostgreSQL concurrency and Testcontainers evidence remain open.

The fifth M3 slice publishes `PartitionAdmissionPolicy`, measured baseline/shadow
evidence types, typed rejection reasons, and `PartitionShadowPlan`. Admission is
fail-closed unless the packet passes minimum row/byte/tenant scale, tenant-predicate
coverage, entity/link digest parity, catch-up, foreign-key/orphan checks, query-plan
stability, p95 query/mutation regression, WAL amplification, partition-size skew,
and cutover-lock limits. Tenant-hash modulus is restricted to powers of two from 2
through 128. Admitted plans derive stable SHA-256-bound shadow parent/child names
and emit only shadow bootstrap DDL. They never rename, drop, or alter production
relations. Copy, constraints, indexes, replay, cutover, rollback, and global
operation fencing remain open until retained PostgreSQL evidence exists.

The sixth M3 slice adds immutable partition evidence preparation and validation.
The preparer binds repository, commit, PostgreSQL image, strategy, modulus,
locales, repetitions, and explicit thresholds to one SHA-256 `evidence_id`, then
emits deterministic shadow-only bootstrap SQL without clobbering an existing
manifest. The validator rejects incomplete packets and calculates tenant-predicate
coverage, cardinality/digest parity, normalized plan changes, p95 regressions, WAL
amplification, child-size skew, lock duration, rollback facts, and typed admission
reasons before atomically publishing an outcome. The repository owner still must
execute and retain the PostgreSQL packet. This slice also removes the stale
integration-test assumption that `IndexModule` has no production migrations.

The seventh M3 slice adds fail-closed raw-artifact packet assembly. A strict
`index_partition_capture_v1` descriptor points to exactly six unique relative JSON
files inside one bundle. The assembler rejects absolute paths, traversal,
directories, symbolic links, hard-link aliases, output aliases, and overwrite
attempts. It reads each artifact once, hashes exact bytes, maps only the required
baseline/shadow/query/mutation/maintenance/cutover shapes, and runs the canonical
packet validator before no-clobber publication. It still does not execute
PostgreSQL measurements or authorize production partitioning.

The eighth M3 slice adds an owner-operated PostgreSQL snapshot runner. It requires
an explicit shadow-copy opt-in, PostgreSQL 16 with JIT disabled, ordinary
unpartitioned canonical relations, a deterministic prepared manifest, and a
reviewed tenant-predicate audit. It serializes one evidence ID with an advisory
lock, creates only evidence-bound tenant-hash shadow parents and children, and
copies entities and links from one repeatable-read snapshot. A shadow-only
source-version unique index and validated source foreign key protect link parity.
The runner records baseline/shadow rows, physical bytes, logical SHA-256 digests,
child sizes, orphan state, FK state, and post-copy catch-up, then publishes
`baseline.json` and `shadow.json` together without overwriting retained evidence.
It does not execute query, mutation, maintenance, or cutover measurements and never
renames, drops, or alters the canonical production relations.

The ninth M3 slice publishes `PostgresSchemaRegistrationStore`, the generic
source-to-Index schema persistence boundary. Registration validates the owner
schema and tenant, calculates the canonical fingerprint and semantic JSON, and on
PostgreSQL serializes one tenant/module/entity identity with a transaction advisory
lock. Exact active re-registration is `Unchanged`; a new greater version is
`Inserted`. Same-version contract reuse, a missing lower version after a newer
version, retired reactivation, nil tenant, malformed persisted state, unsupported
backend, and storage failure fail closed. A conflict race is verified after
`ON CONFLICT DO NOTHING`. SQLite provides contract-test coverage. Source modules
never import Index entities or write `index_schemas` directly.

### M4 - Query engine v1

- [ ] Produce deterministic executable query plans from validated queries.
- [ ] Resolve explicit link paths and assign stable aliases.
- [ ] Compile plans through SeaQuery or controlled SQL.
- [ ] Support nested projection, filtering, sorting, exact count, and keyset
      pagination.
- [ ] Keep offset pagination bounded and compatibility-only.
- [ ] Add plan/SQL snapshots and PostgreSQL/reference-engine equivalence tests.

### M5 - Incremental ingestion

- [ ] Add source-provider and mutation runtime registries over persisted schemas.
- [x] Add inbox deduplication and monotonic source versions.
- [ ] Add batch transactions, retry classification, backoff, dead-letter state,
      and lag metrics.
- [x] Protect single-record mutation storage against out-of-order update/delete delivery.
- [ ] Cover crash between commit and acknowledgement in a composed worker.

### M6 - Rebuild and reconciliation

- [ ] Add cursor-based `IndexSource::scan` and targeted `load`.
- [ ] Add durable jobs, checkpoints, leases, heartbeat, and ownership.
- [ ] Add bounded streaming; never collect all IDs first.
- [ ] Add cancellation, resume, dry-run, targeted/full/shadow rebuild.
- [ ] Add reconciliation and drift repair.
- [ ] Cover crash, lease expiry, restart, cancellation, and incremental/full
      rebuild equivalence.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [ ] Register owner-published schemas and links through the persisted owner API.
- [ ] Implement mutations and rebuild sources.
- [ ] Support tenant, locale, status, projection, link filters, sorting, and
      cursor pagination.
- [ ] Move one Storefront query to Index.
- [ ] Prove no source-module filtering fan-out.

The Social Graph relation projection is an earlier bounded infrastructure consumer
used to prove sealed-event conversion, tenant schema registration, mutation inbox
semantics, and result-first acknowledgement. It does not replace the Product-first
query vertical and is never an authorization source.

### M8 - Commerce scale slice

- [ ] Add Pricing, Inventory, Category, Collection, Tags, Region/Currency, and
      Marketplace Seller schemas/sources.
- [ ] Filter by price, stock, category, channel, and seller in one query.
- [ ] Add cardinality and load benchmarks.

### M9 - Content, Flex, and extension schemas

- [ ] Add Content, Pages, Blog, Forum, Taxonomy, SEO, and Flex schemas.
- [ ] Make Flex use ordinary dynamic schema/source registration.
- [ ] Prove a new module requires no Index-core code changes.

### M10 - Horizontal scaling

- [ ] Test multiple workers/server instances, concurrent schema registration and
      application, rebuild, redelivery, slow sources, connection loss, tenant
      hotspots, and backpressure.
- [ ] Add graceful shutdown and task-ownership evidence.
- [ ] Split core/postgres/worker crates only when measurements justify it.

### M11 - Admin and cutover

- [ ] Expose schema, partition, lag, inbox, failure, rebuild, drift, and query
      diagnostics.
- [ ] Add rebuild/cancel/retry commands.
- [ ] Publish new FBA contracts and runtime evidence.
- [ ] Migrate consumers and delete final compatibility code.
- [ ] Promote FBA only after compiled/live evidence.

## Verification

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo xtask module validate index
cargo xtask module test index
npm run verify:index:fba
npm run verify:index:runtime-fallback-smoke
node scripts/verify/index-storage-tooling.mjs contract
node scripts/verify/index-storage-tooling.mjs fixtures
node --test scripts/verify/compare-index-storage-evidence.test.mjs
node --test scripts/verify/index-partition-evidence.test.mjs
node --test scripts/verify/index-partition-evidence-assembly.test.mjs
cargo check -p rustok-benchmarks --bin index-partition-snapshot-capture
cargo test -p rustok-benchmarks partition_snapshot
```

Targeted M3 maintainer checks:

```bash
cargo check -p rustok-index --all-targets
cargo test -p rustok-index --lib
cargo test -p rustok-index --test module
node scripts/verify/verify-index-mutation-storage.mjs
node scripts/verify/verify-index-schema-registration.mjs
node scripts/verify/verify-index-schema-leases.mjs
node scripts/verify/verify-index-secondary-index-lifecycle.mjs
node scripts/verify/verify-index-partition-admission.mjs
node scripts/verify/verify-index-partition-evidence.mjs
node scripts/verify/verify-index-partition-snapshot-capture.mjs
```

## Progress log

- 2026-07-23: completed the destructive reset and generic M1 core.
- 2026-07-24 through 2026-07-27: completed the deterministic M2 PostgreSQL
  comparison, archived same-commit replacement evidence, accepted JSONB, and
  removed rejected prototypes.
- 2026-07-27: registered the canonical M3 storage schema and added atomic
  mutation/inbox/entity/link persistence.
- 2026-07-27: added generic tenant-scoped persisted source schema registration,
  exact re-registration, version/conflict/retired guards, owner-neutral tests,
  documentation, and a static boundary verifier.
- 2026-07-27: added durable schema-application exclusion, expiry reclaim,
  heartbeat, terminal completion, and attempt fencing.
- 2026-07-27: added schema-derived typed/containment secondary-index planning,
  concurrent ensure/reindex/retire execution, catalog readiness checks, durable
  jobs, owner fingerprints, and operation fencing.
- 2026-07-27: added fail-closed measured partition admission and deterministic
  tenant-hash shadow planning without destructive production cutover SQL.
- 2026-07-27: added immutable partition evidence manifests, calculated packet
  admission, non-clobbering shadow bootstrap publication, owner runbook, lightweight
  CI guards, and corrected the stale integration migration contract.
- 2026-07-27: added exact-byte raw-artifact capture assembly, bundle confinement,
  no-symlink/no-alias/no-overwrite publication, fixture coverage, and tooling
  routing.
- 2026-07-27: added owner-operated PostgreSQL baseline/shadow snapshot capture,
  repeatable-read copy, shadow integrity validation, logical SHA-256 parity, child
  size evidence, no-clobber pair publication, and static/CI guards. Repository
  tests, verifiers, and real PostgreSQL evidence remain for the owner to execute.
