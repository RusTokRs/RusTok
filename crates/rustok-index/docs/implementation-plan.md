# Implementation plan for `rustok-index`

## Mission

`rustok-index` is the platform-owned cross-module relational index and query engine.
Source modules publish generic schemas, records, mutations, and links; Index
materializes them into PostgreSQL and executes structured filtering, projection,
sorting, counting, and pagination without runtime fan-out to source tables.

`rustok-index` is not a search engine. Ranking, relevance, typo tolerance, synonyms,
autocomplete, search UX, and external search-engine connectors remain owned by
`rustok-search`.

## Rewrite policy

The project is in early development. Backward compatibility with the rejected
implementation is not a goal. Prefer clean replacement over compatibility layers.
Index core must not import source-domain semantics or query source-module tables.
Benchmark/evidence code remains outside production migrations and runtime
composition. Partition cutover remains forbidden until one retained real
PostgreSQL packet satisfies the explicit admission policy.

The repository owner performs test, benchmark, and evidence execution. Commits and
pull requests record checks and PostgreSQL runs that were not executed.

## Current state

- Rewrite status: `in_progress`
- Current milestone: `M4 - Query engine v1 (planning started; M3 retained packet owner gate remains open)`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: `complete`
- M1 domain/application core: `complete`
- M2 storage benchmark: `complete`
- M2 storage decision: `JSONB accepted; rejected prototypes removed`
- M3 storage-schema foundation: `complete`
- M3 atomic mutation persistence: `complete`
- M3 schema-application leases: `complete`
- M3 secondary-index lifecycle: `complete`
- M3 partition admission and shadow planning: `complete`
- M3 partition evidence packet tooling: `complete`
- M3 partition evidence capture/assembly: `complete`
- M3 partition baseline/shadow snapshot runner: `complete`
- M3 partition query evidence runner: `complete`
- M3 partition mutation/WAL evidence runner: `complete`
- M3 partition maintenance evidence runner: `complete`
- M3 partition cutover rehearsal evidence runner: `complete`
- M3 retained packet owner orchestration: `complete`
- M3 retained bundle review/report: `complete`
- M3 admitted archive manifest: `complete`
- M3 retained archive verification and filesystem snapshot: `complete`
- Real retained PostgreSQL packet execution: `open`
- M4 deterministic executable query planning: `complete`
- M4 stable explicit-link relation aliases: `complete`
- Production persistence: mutation writes, schema/index coordination, fail-closed
  partition admission, snapshot/query/mutation/maintenance/cutover evidence tooling,
  full-capture orchestration, exact-byte packet assembly, retained bundle review,
  admitted archive manifest generation, saved-manifest verification, recursive
  filesystem integrity checks, and database-independent query planning are
  implemented; one retained admitted packet, SQL query adapter, and production
  partition lifecycle remain open

The production crate contains the generic domain/application core, seven canonical
M3 tables, an atomic mutation adapter, durable schema leases, secondary-index
lifecycle management, measured partition admission that emits shadow bootstrap
plans only, and an M4 structural query planner with deterministic relation aliases.
Owner-operated evidence tools live under `ops/benches`; they do not become runtime
storage code.

## Ownership

Index owns schema/link registration, generic records and mutations, ingestion,
inbox deduplication, PostgreSQL storage, query validation/planning/compilation,
filtering, projection, sorting, counting, pagination, rebuild, checkpointing,
reconciliation, drift repair, distributed coordination, and operator diagnostics.

Source modules own normalized domain data, schema declarations, conversion to
generic Index records/mutations, rebuild scan/load adapters, and source ordering and
version information.

## Target architecture

```text
source modules
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
    planner.rs
  migrations/
  infrastructure/postgres/
    mutation_store.rs
    schema_lease.rs
    secondary_index.rs
    partition_admission.rs
  api/

ops/benches/src/index_storage/
  runner.rs
  mutation_runner.rs
  maintenance_runner.rs
  partition_snapshot.rs
  partition_query.rs
  partition_mutation.rs
  partition_maintenance.rs
  partition_cutover.rs
  partition_capture.rs
```

## Library decisions

Use workspace `sea-orm`/SeaQuery for PostgreSQL, `tokio`/`futures-util` for bounded
async work, `serde`/`postcard` for contracts, `thiserror` for typed failures,
`tracing`/telemetry/prometheus for observability, `petgraph` for deterministic graph
resolution, ICU4X for locale canonicalization, and `sha2` for schema, cursor, plan,
and retained-evidence identities. Add Testcontainers, retry, cancellation, or
snapshot libraries only when their slices require them.

Forbidden in Index core: source-domain dependencies, ranking/search libraries, a
second database stack, unvalidated JSON-only public queries, unbounded rebuild ID
collection, direct source-table reads, and destructive partition cutover without
retained evidence and rollback proof.

## Milestones

### M0 - Hard reset and architecture lock

- [x] Replace the implementation with the generic Index Engine roadmap.
- [x] Record ownership and rewrite policy in an ADR.
- [x] Remove legacy source-specific ports, adapters, migrations, scheduling, server
      composition, and admin table reads.
- [x] Remove source-domain dependencies and add guards preventing their return.

### M1 - Domain core and schema registry

- [x] Add bounded identifiers, canonical locales, schema identities, and versions.
- [x] Add `IndexValue`, `IndexRecord`, `IndexMutation`, `IndexSchema`, and links.
- [x] Add stable order-independent SHA-256 schema fingerprints.
- [x] Add atomic registration and deterministic link-path resolution.
- [x] Validate records, mutations, queries, types, operators, cardinality, tenant/
      locale scope, and complexity.
- [x] Add checksummed query-scoped keyset cursors.
- [x] Add a test-only reference engine and property invariants.

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
decision. Partitioning was not measured by M2, so canonical production tables remain
unpartitioned until separate shadow evidence is retained and admitted.

### M3 - PostgreSQL storage engine

- [x] Add canonical schema/entity/link/inbox/job/checkpoint/consistency migrations.
- [x] Add tenant/schema/entity/locale keys and source-version guards.
- [x] Add atomic entity/link upsert and delete transactions.
- [x] Add schema-application locks and leases.
- [x] Add schema-derived secondary-index planning and lifecycle management.
- [x] Add fail-closed partition admission and deterministic tenant-hash shadow plans.
- [x] Add immutable partition manifests, measured packet validation, and runbook.
- [x] Add exact-byte raw-artifact assembly with confinement and no-clobber output.
- [x] Add owner-operated PostgreSQL baseline/shadow snapshot capture.
- [x] Add owner-operated PostgreSQL baseline/shadow query evidence capture.
- [x] Add owner-operated PostgreSQL baseline/shadow mutation and WAL evidence capture.
- [x] Add owner-operated PostgreSQL baseline/shadow ordinary-VACUUM maintenance evidence capture.
- [x] Add owner-operated PostgreSQL cutover/rollback rehearsal evidence capture.
- [x] Add owner-operated full retained packet orchestration and capture finalization.
- [x] Add read-only retained bundle review with recalculated assembly and admission.
- [x] Add admitted archive manifest and saved-manifest verification receipt.
- [x] Bind retained verification to stable descriptors, filesystem identity and
      fingerprints, and an exact recursive directory inventory.
- [ ] Execute one fresh full PostgreSQL capture and retain all six raw artifacts,
      `capture.json`, `partition-packet.json`, and `admission.json`.
- [ ] Review and archive one complete admitted real packet before production lifecycle
      design proceeds.
- [ ] Add partition copy/checkpoints, constraints/index attachment, replay/dual-write,
      cutover, rollback, and durable global operation ownership.
- [ ] Add PostgreSQL Testcontainers fixtures.
- [ ] Cover migration-from-zero, stale mutation, redelivery, rollback, concurrency,
      and tenant/locale isolation in PostgreSQL.

#### Retained repository contract wording

The following wording remains explicit because repository guards bind the completed
slices and still-open owner evidence to these exact architectural boundaries:

- [x] Add locking/leases for schema application.
- [x] Add secondary-index planning and lifecycle management.
- [x] Add tenant/schema/entity/locale keys and source-version guards.
- [x] Add immutable partition evidence manifest, measured packet validator, and
      owner-operated runbook.
- [x] Add exact-byte raw-artifact capture assembly with bundle confinement and
      no-clobber packet publication.
- [x] Add read-only retained bundle review, admitted archive manifest, and
      saved-manifest verification receipt.
- [x] Bind retained verification to an exact recursive filesystem snapshot and refuse
      post-inspection drift.
- [ ] Execute retained PostgreSQL partition baseline/shadow evidence.
- [ ] Execute retained PostgreSQL query, mutation, maintenance, and cutover evidence.
- [ ] Execute retained PostgreSQL mutation, maintenance, and cutover evidence.

Partition admission remains fail-closed across tenant-predicate coverage,
query/mutation latency and plan stability, WAL amplification, skew, and cutover lock
evidence. Admitted plans emit shadow-only DDL. They never rename, drop, or alter production
relations.

#### Completed M3 slices

1. Canonical migrations register the seven module-owned tables with complete tenant,
   schema, entity, locale, and full-range non-negative source-version identity.
2. `PostgresMutationStore` validates deliveries, claims the composite inbox key,
   serializes the entity key, and applies entity/tombstone/link replacement atomically.
3. `PostgresSchemaLeaseStore` provides durable schema-application exclusion,
   reclaim, heartbeat, terminal completion, and attempt fencing.
4. `SecondaryIndexPlan` and `PostgresSecondaryIndexManager` derive typed/GIN indexes,
   coordinate concurrent ensure/reindex/retire work, and verify catalog readiness.
5. `PartitionAdmissionPolicy` and `PartitionShadowPlan` implement measured,
   fail-closed admission and deterministic shadow-only hash partition DDL.
6. Immutable manifest preparation and packet validation bind commit, image, modulus,
   repetitions, thresholds, evidence identity, raw hashes, and calculated reasons.
7. `index_partition_capture_v1` assembly reads six unique confined raw files once,
   calculates exact-byte hashes, validates the packet, and refuses overwrite/aliases.
8. The snapshot runner creates evidence-bound shadow parents/children, copies one
   repeatable-read baseline, attaches shadow integrity, records parity/size/catch-up,
   and publishes `baseline.json` plus `shadow.json` without touching canonical DDL.
9. The query runner validates the shadow catalog, executes exact tenant-scoped runs
   read-only, proves result parity and one-child pruning, retains full EXPLAIN JSON,
   calculates p95 and normalized plan digests, and publishes `query.json` once.
10. The mutation/WAL runner validates the same manifest and catalog, requires count
    parity and matching generic anchors, executes rollback-only mutation samples,
    proves one-child shadow pruning, retains EXPLAIN JSON and WAL evidence, and
    publishes `mutation.json` without overwrite. Real database execution remains an
    owner step.
11. The maintenance runner revalidates the manifest and retained shadow catalog,
    creates isolated ordinary and tenant-hash clone pairs, applies identical committed
    churn only to those clones, times ordinary `VACUUM (ANALYZE)`, proves production
    and retained snapshot-shadow relations unchanged, and publishes
    `maintenance.json` without overwrite. Real database execution remains an owner
    step.
12. The cutover rehearsal runner validates production and retained shadow identities,
    creates deterministic evidence-only ordinary clones, measures `ACCESS EXCLUSIVE`
    lock acquisition, performs rename swaps only inside rollback-only transactions,
    proves clone OIDs/names and production relations unchanged, and publishes
    `cutover.json` without overwrite. Real database execution remains an owner step.
13. The full-capture orchestrator requires one explicit owner opt-in, one immutable
    manifest, one database URL, and a fresh empty output directory. It sequentially
    runs all five evidence commands, finalizes `capture.json` with PostgreSQL identity
    and run provenance, assembles exact retained bytes into `partition-packet.json`,
    validates `admission.json`, and refuses partial-output reuse or resume.
14. The retained bundle review inspects exactly nine authoritative files, recalculates
    exact-byte packet assembly and admission, rejects aliases or drift, and renders a
    deterministic read-only owner report without editing the bundle.
15. The archive tooling emits a deterministic admitted-only manifest outside the
    immutable bundle and verifies a saved manifest into a read-only receipt with
    `production_lifecycle_authorized: false`.
16. Retained verification binds every authoritative file and required directory to
    stable-descriptor bytes, filesystem identity, metadata fingerprints, canonical
    paths, and an exact recursive inventory. It rereads the saved manifest and fails
    closed on replacement, metadata, inventory, alias, or post-inspection byte drift
    while keeping public manifest and receipt schemas unchanged.

### M4 - Query engine v1

- [x] Produce deterministic executable plans from validated queries.
- [x] Resolve explicit link paths and assign stable aliases.
- [ ] Compile plans through SeaQuery or controlled SQL.
- [ ] Support nested projection, filtering, sorting, exact count, and keyset
      pagination.
- [ ] Keep offset pagination bounded and compatibility-only.
- [ ] Add plan/SQL snapshots and PostgreSQL/reference-engine equivalence tests.

The first M4 slice is database independent. `SchemaRegistry::plan_query` validates
before planning, assigns stable `t0`, `t1`, ... aliases from sorted explicit link
prefixes, binds projection and ordering to those aliases, retains typed filters and
pagination for the compiler, and publishes a versioned SHA-256 plan fingerprint.
SQL execution, cursor predicate compilation, query-port composition, and consumer
cutover remain open.

### M5 - Incremental ingestion

- [ ] Add source and mutation registries.
- [ ] Add inbox deduplication and monotonic source versions.
- [ ] Add batch transactions, retry classification, backoff, dead-letter state, and
      lag metrics.
- [ ] Protect against out-of-order update/delete delivery.
- [ ] Cover crash between commit and acknowledgement.

### M6 - Rebuild and reconciliation

- [ ] Add cursor-based `IndexSource::scan` and targeted `load`.
- [ ] Add durable jobs, checkpoints, leases, heartbeat, and ownership.
- [ ] Add bounded streaming; never collect all IDs first.
- [ ] Add cancellation, resume, dry-run, targeted/full/shadow rebuild.
- [ ] Add reconciliation and drift repair.
- [ ] Cover crash, lease expiry, restart, cancellation, and incremental/full
      equivalence.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [ ] Register owner-published schemas and links.
- [ ] Implement mutations and rebuild sources.
- [ ] Support tenant, locale, status, projection, link filters, sorting, and cursor
      pagination.
- [ ] Move one Storefront query to Index.
- [ ] Prove no source-module filtering fan-out.

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

- [ ] Test multiple workers/instances, concurrent schema application and rebuild,
      redelivery, slow sources, connection loss, hotspots, and backpressure.
- [ ] Add graceful shutdown and task-ownership evidence.
- [ ] Split crates only when measurements justify it.

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
cargo test -p rustok-index planner_tests -- --nocapture
cargo xtask module validate index
cargo xtask module test index
npm run verify:index:fba
npm run verify:index:runtime-fallback-smoke
node scripts/verify/index-storage-tooling.mjs contract
node scripts/verify/index-storage-tooling.mjs fixtures
node --test scripts/verify/index-partition-evidence.test.mjs
node --test scripts/verify/index-partition-evidence-assembly.test.mjs
cargo check -p rustok-benchmarks --bin index-partition-snapshot-capture
cargo test -p rustok-benchmarks partition_snapshot
cargo check -p rustok-benchmarks --bin index-partition-query-evidence
cargo test -p rustok-benchmarks partition_query
cargo check -p rustok-benchmarks --bin index-partition-mutation-evidence
cargo test -p rustok-benchmarks partition_mutation
cargo check -p rustok-benchmarks --bin index-partition-maintenance-evidence
cargo test -p rustok-benchmarks partition_maintenance
cargo check -p rustok-benchmarks --bin index-partition-cutover-evidence
cargo test -p rustok-benchmarks partition_cutover
cargo check -p rustok-benchmarks --bin index-partition-capture-finalize
```

Targeted M3/M4 guards:

```bash
node scripts/verify/verify-index-mutation-storage.mjs
node scripts/verify/verify-index-schema-leases.mjs
node scripts/verify/verify-index-secondary-index-lifecycle.mjs
node scripts/verify/verify-index-partition-admission.mjs
node scripts/verify/verify-index-partition-evidence.mjs
node scripts/verify/verify-index-partition-snapshot-capture.mjs
node scripts/verify/verify-index-partition-query-evidence.mjs
node scripts/verify/verify-index-partition-mutation-evidence.mjs
node scripts/verify/verify-index-partition-maintenance-evidence.mjs
node scripts/verify/verify-index-partition-cutover-evidence.mjs
node scripts/verify/verify-index-partition-full-capture.mjs
node scripts/verify/verify-index-partition-post-inspection-drift.mjs
node scripts/verify/verify-index-query-planner.mjs
```

## Progress log

- 2026-07-23: completed the destructive reset and generic M1 core.
- 2026-07-24 through 2026-07-27: completed M2 comparison, archived replacement
  evidence, accepted JSONB, and removed rejected prototypes.
- 2026-07-27: completed canonical M3 schema, atomic mutation persistence, schema
  leases, secondary-index lifecycle, partition admission/planning, immutable packet
  tooling, exact-byte assembly, baseline/shadow snapshot capture, query evidence
  capture, rollback-only mutation/WAL evidence capture, and isolated ordinary-VACUUM
  maintenance evidence capture.
- 2026-07-28: completed rollback-only cutover rehearsal evidence, full retained packet
  owner orchestration with PostgreSQL identity-bound capture finalization, read-only
  retained bundle review, admitted archive manifest generation, saved-manifest
  verification receipts, and exact recursive filesystem snapshot enforcement.
- 2026-07-29: rechecked the merged M3 source boundary and completed the first M4
  source slice: validated structural plans, deterministic explicit-link aliases, and
  a versioned query-plan fingerprint. SQL compilation remains the next bounded slice.
- Repository test/fixture suites, verifiers, and one real full PostgreSQL partition
  packet remain for the owner to execute and admit before production partition
  lifecycle work begins.
