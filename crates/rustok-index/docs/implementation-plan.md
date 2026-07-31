# Implementation plan for `rustok-index`

## Mission

`rustok-index` is the platform-owned cross-module relational index and query engine.
Source modules publish generic schemas, records, mutations, links, and bounded replay
sources; Index materializes them into PostgreSQL and executes structured filtering,
projection, sorting, counting, and pagination without runtime fan-out to source tables.

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
- Current milestone: `M7 - Product/ProductVariant/SalesChannel bounded replay graph (live sources, Product/ProductVariant/SalesChannel tombstones, and bounded reconciliation source-complete; incremental events, persisted readiness, durable Product-to-Channel relations, consumer cutover, and retained evidence open)`
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
- M4 controlled PostgreSQL query compilation: `complete`
- M4 typed root/one-link query semantics: `complete`
- M4 deterministic PostgreSQL result decoding: `complete`
- M4 many-link `EXISTS` filtering: `complete`
- M4 nested many-link projection aggregation: `complete`
- M4 PostgreSQL query port and strict row adapter: `source_complete`
- M4 retained plan/SQL snapshots: `source_complete`
- M4 explicit many-link aggregate ordering: `source_complete`
- M4 source-owned schema catalog and shared query runtime: `source_complete`
- M4 first Social Graph privacy parity shadow: `source_complete_metrics_evidence_tooling_execution_pending`
- M5 inbox deduplication and monotonic source-version persistence: `complete`
- M5/M6 bounded source replay contract: `source_complete_owner_execution_pending`
- M6 one-page replay and durable checkpoint progression: `source_complete`
- M6 replay job leases and checkpoint attempt fencing: `source_complete_owner_execution_pending`
- M6 bounded multi-page replay and cancellation: `source_complete_owner_execution_pending`
- M6 bounded multi-pass source reconciliation: `source_complete_owner_execution_pending`
- M6 replay runtime host composition and operator guard: `source_complete_owner_execution_pending`
- M7 Product/ProductVariant graph schemas and bounded sources: `source_complete_owner_execution_pending`
- M7 SalesChannel schema, bounded source, and retained deletes: `source_complete_owner_execution_pending`
- Production persistence: mutation writes, schema/index coordination, fail-closed
  partition admission, snapshot/query/mutation/maintenance/cutover evidence tooling,
  full-capture orchestration, exact-byte packet assembly, retained bundle review,
  admitted archive manifest generation, saved-manifest verification, recursive
  filesystem integrity checks, typed executable query planning, controlled projection,
  root/one-link filtering, ordering, exact-count, keyset, bounded-offset SQL
  compilation, correlated many-link filtering, deterministic nested many-link
  projection aggregation, explicit bounded many-link MIN/MAX ordering, one-row page
  lookahead, tagged row decoding, exact-count decoding, scoped next-cursor construction,
  persisted-schema query preflight, PostgreSQL page/count execution, compiler-driven
  row adaptation, source-owned schema publication, shared query-runtime composition,
  bounded source replay/load contracts, one-page replay mutation application, durable
  rebuild checkpoint progression, schema-scoped rebuild job claims, lease/heartbeat,
  attempt fencing, fenced checkpoint writes, bounded multi-page execution, immediate
  pending resume, durable cancellation requests, fenced between-page terminal
  cancellation, bounded multi-pass reconciliation, immutable source-registry host
  materialization, shared replay-runtime publication, request-bound operator
  authorization, atomic PostgreSQL source-factory composition, stable source event
  identities, Product/ProductVariant/SalesChannel current-state sources, versioned
  Product-to-ProductVariant links, and retained Product/ProductVariant/SalesChannel
  hard-delete mutations are implemented; one retained admitted packet, live
  PostgreSQL/reference equivalence, in-page interruption, retry/backoff, dead-letter
  scheduling, host scheduling, graceful task shutdown, command transport, incremental
  event acknowledgement, persisted per-tenant schema readiness, durable
  Product-to-SalesChannel relations, freshness/recovery evidence, authoritative
  consumer cutover, tombstone purge admission, and production partition lifecycle
  remain open.

The production crate contains the generic domain/application core, seven canonical
M3 tables, an atomic mutation adapter, durable schema leases, secondary-index
lifecycle management, measured partition admission that emits shadow bootstrap
plans only, an M4 typed structural query planner with deterministic relation aliases,
a controlled PostgreSQL compiler for root/one-link projection, filtering, ordering,
exact count, keyset and bounded offset plus duplicate-free correlated many-link
filtering, deterministic nested many-link projection aggregates, and explicit
many-link MIN/MAX aggregate ordering, a strict compiled-row decoder with aligned
nested identities/values, lookahead pagination, and scoped cursor construction,
`PostgresIndexQueryPort` for exact-schema-preflighted execution in one read-only
repeatable-read page/count snapshot, source-owned schema and replay catalogs, a
neutral bounded `IndexSource` scan/load boundary, a one-page replay executor,
`PostgresIndexReplayJobStore` for durable fenced schema-scoped rebuild ownership,
a lease-bound PostgreSQL rebuild-checkpoint adapter, `PostgresIndexReplayRunner` for
bounded heartbeat/yield/cancellation execution, a bounded multi-pass reconciliation
runner with durable pass/source cursor state, `SharedIndexReplayRuntime` for host
capability transfer, and atomic host-database-aware source factory composition. The
server publishes `IndexReplayOperatorRuntime` as the request-bound authorization
boundary. The selected distribution contributes Product, ProductVariant, and
SalesChannel schemas/sources without adding source-domain dependencies to Index core
or server. Owner-operated evidence tools live under `ops/benches`; they do not become
runtime storage code.

## Ownership

Index owns schema/link/source registration, generic records and mutations, ingestion,
inbox deduplication, PostgreSQL storage, query validation/planning/compilation,
filtering, projection, sorting, counting, pagination, rebuild, checkpointing,
reconciliation, drift repair, distributed coordination, and operator diagnostics.

Source modules own normalized domain data, schema declarations, conversion to
generic Index records/mutations, bounded `IndexSource::scan` and targeted `load`
adapters, stable replay event identities, source ordering and version information,
and retained deletion identities needed for replay. Selected cross-module adapters
that require both owner storage and Index contracts live in `rustok-distribution`;
they do not move storage ownership into Index or server.

## Target architecture

```text
source modules
    -> IndexSchemaSourceCatalog / IndexSourceCatalog / IndexMutation
    -> ingestion, rebuild, and reconciliation engines
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
    postgres_compiler.rs
    postgres_query_sql.rs
    postgres_query_result.rs
    query_port.rs
    source_event_id.rs
    source_schema_registry.rs
    source_registry.rs
    source_replay.rs
  migrations/
  infrastructure/postgres/
    mutation_store.rs
    source_factory.rs
    source_replay.rs
    source_replay_job.rs
    source_replay_runner.rs
    source_reconciliation.rs
    replay_runtime.rs
    schema_lease.rs
    secondary_index.rs
    partition_admission.rs
    query_port.rs
  api/

crates/rustok-distribution/src/
  product_index/
  channel_index.rs

apps/server/src/services/
  index_replay_runtime_composition.rs

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
source-event, and retained-evidence identities. Add Testcontainers, retry,
cancellation, or snapshot libraries only when their slices require them.

Forbidden in Index core: source-domain dependencies, ranking/search libraries, a
second database stack, unvalidated JSON-only public queries, unbounded rebuild ID
collection, direct source-table reads, source-owned writes to Index tables, and
destructive partition cutover without retained evidence and rollback proof.

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
- [x] Add owner-operated PostgreSQL ordinary-VACUUM maintenance evidence capture.
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

Canonical migrations, atomic mutation persistence, schema leases, secondary-index
lifecycle, fail-closed partition admission, exact-byte retained packet tooling,
baseline/shadow capture, query/mutation/WAL/maintenance/cutover evidence runners,
full-capture orchestration, read-only retained review, admitted archive manifests,
and recursive filesystem integrity checks are source complete. Real PostgreSQL
execution and admission remain owner steps.

### M4 - Query engine v1

- [x] Produce deterministic executable plans from validated queries.
- [x] Resolve explicit link paths and assign stable aliases.
- [x] Capture typed referenced-field contracts in executable plans.
- [x] Compile plans through SeaQuery or controlled SQL.
- [x] Support root and one-cardinality-link projection, filtering, sorting, exact
      count, and keyset pagination.
- [x] Keep offset pagination bounded and compatibility-only.
- [x] Add deterministic root/one-link result decoding and cursor construction.
- [x] Add explicit many-link `EXISTS` filtering.
- [x] Add nested many-link projection aggregation.
- [x] Add PostgreSQL `IndexQueryPort` execution and strict SeaORM row adaptation.
- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.
- [x] Add explicit many-link MIN/MAX aggregate ordering for integer, string, timestamp,
      and Decimal terminals under bounded offset pagination.
- [x] Publish source-owned schemas, compose one shared query runtime, and stage the first
      default-off owner-authoritative Social Graph privacy parity shadow.
- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.

The first M4 slice is database independent. `SchemaRegistry::plan_query` validates
before planning, assigns stable `t0`, `t1`, ... aliases from sorted explicit link
prefixes, and captures every referenced field with type, cardinality, nullability,
path, alias, and propagated many-link traversal state. The typed plan fingerprint
uses the versioned `rustok-index-query-plan-v4` domain.

The controlled compiler emits ordered typed binds, deterministic identity/projection/
order columns, exact tenant/schema/locale/live-row scope, all current typed filters,
reference-compatible null ordering, validated lexicographic keyset predicates with an
ascending root `entity_id` tie-breaker, bounded offset pagination, and an optional
separate exact-count statement without pagination leakage.

Each selected many path compiles as one correlated JSONB aggregate outside the outer
rowset. Many aggregates do not alter root pagination, lookahead, or exact-count
cardinality. Many-link filtering compiles every atomic predicate through an
independent nested correlated `EXISTS` chain, so root pagination and exact count stay
duplicate free.

`PostgresIndexQueryPort` requires exact persisted schema readiness and executes page
and optional count in one read-only repeatable-read transaction. Live metrics/evidence
execution, PostgreSQL/reference-engine equivalence admission, and authoritative
consumer cutover remain open.

### M5 - Incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [ ] Add a mutation-source event registry and broker acknowledgement orchestration.
- [x] Add inbox deduplication and monotonic source versions.
- [ ] Add batch transactions, retry classification, bounded backoff, dead-letter state,
      and lag metrics around the source contracts.
- [ ] Protect the complete event-to-ack path against out-of-order update/delete delivery.
- [ ] Cover crash between commit and acknowledgement.

The source registry fixes one replay source for every exact schema and the complete
schema identity across versions. Materialization requires every replay schema to exist
in the source-owned schema catalog with the same module owner. Source failures expose
only bounded machine-readable codes classified as retryable or permanent; raw source,
database, or transport causes stay in owner logs.

### M6 - Rebuild and reconciliation

- [x] Add cursor-based `IndexSource::scan` and targeted `load` contracts.
- [x] Bound cursor bytes, scan pages, targeted key counts, tenant/schema scope,
      uniqueness, and continuation progress; never collect all IDs first.
- [x] Add a durable rebuild checkpoint read/write adapter over `index_checkpoints`.
- [x] Add a bounded worker that applies source pages through `PostgresMutationStore` and
      commits checkpoints only after durable mutation results.
- [x] Reject nil/duplicate replay event UUIDs and require stable event identity across
      retries for the same logical mutation and source version.
- [x] Add durable schema-scoped rebuild jobs, lease/heartbeat, reclaim, attempt fencing,
      exact request validation, and terminal-state fencing.
- [x] Bind checkpoint reads and writes to the active `(job_id, worker_id, attempt_count)`
      and require a durable null-cursor checkpoint before terminal success.
- [x] Add bounded multi-page execution with heartbeat cadence and immediate pending resume.
- [x] Add durable cancellation requests and fenced between-page terminal cancellation.
- [x] Bind job requests directly to the materialized source registry in server composition.
- [x] Add bounded multi-pass source reconciliation with durable pass/source cursor state.
- [ ] Add in-page interruption/timeouts, dry-run, and targeted/full/shadow rebuild modes.
- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.
- [ ] Add locale/partition replay checkpoint dimensions.
- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.
- [ ] Cover crash, lease expiry, restart, cancellation, authorization, and incremental/full
      equivalence with retained PostgreSQL evidence.

The one-page executor, replay job store, checkpoint adapter, bounded multi-page runner,
cancellation path, reconciliation runner, and host composition are source complete.
`PostgresIndexReplayJobStore` serializes one tenant/schema claim, validates the exact
request and active persisted schema, reclaims expired attempts with an incremented
fence, and rejects stale heartbeat or terminal updates.

`PostgresIndexReplayRunner` resolves the source from the materialized registry, executes
at most 1024 pages, heartbeats between pages, completes only after a durable JSON null
cursor, and returns unfinished work to immediately claimable `pending` state while
preserving the same job UUID and checkpoint.

The bounded reconciliation runner performs durable multi-pass source scans without
collecting all IDs, persists pass and source cursor state, applies stable source
mutations through the same monotonic mutation store, and can be cancelled or reclaimed
under the existing attempt fence. It repairs replay-visible stale and missing materialized
state but does not claim an owner snapshot/high-watermark, a proof against every
concurrent final-pass write, or an authoritative drift-repair admission.

The server materializes `SharedIndexSourceRegistry` only after selected modules register
their source-owned schema and replay contracts. `IndexReplayOperatorRuntime` requires an
exact request-bound tenant/actor permission snapshot and `modules:manage`. In-page
interruption, automatic retry/backoff, dead-letter scheduling, host scheduling, graceful
task shutdown, command/audit transports, and retained multi-instance PostgreSQL evidence
remain open.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [x] Add one locale-required Product scalar schema and bounded PostgreSQL current-state source.
- [x] Add stable `(product_id, locale)` replay cursor, targeted loads, and one-row lookahead.
- [x] Add Product-owned monotonic `index_revision` and stable Index-owned event identity.
- [x] Compose selected Product schema/source through atomic distribution source factories.
- [x] Add Product and translation delete tombstones.
- [x] Add ProductVariant and SalesChannel schemas and bounded sources.
- [x] Add Product-to-ProductVariant v2 links and bounded graph projection fields.
- [x] Add ProductVariant and SalesChannel hard-delete tombstones.
- [ ] Add source-versioned Product, ProductVariant, and SalesChannel incremental event acknowledgement.
- [ ] Add durable Product/ProductVariant-to-SalesChannel relations with owner revision semantics.
- [ ] Support tenant, locale, status, projection, link filters, sorting, and cursor
      pagination across the complete Product/Variant/Channel slice.
- [ ] Move one Storefront query to Index.
- [ ] Prove no source-module filtering fan-out.

Product v1/v2, ProductVariant v1/v2, and SalesChannel v1 schemas are source complete.
Product v2 links to ProductVariant v2; Product channel visibility remains represented by
bounded scalar projection until the owner has a durable relational contract. Product,
translation, ProductVariant, and SalesChannel deletion identities are retained with
monotonic source versions and replayed through the existing stable sources as generic
`IndexMutation::Delete` values. Recreated identities must strictly supersede retained
deletes, and live/tombstone coexistence fails closed.

Product and Channel crates own normalized storage, revisions, and tombstones without
depending on Index. `rustok-distribution` owns the selected generic bridges. Index owns
schema/source/runtime contracts, reconciliation, and mutation persistence. Runtime
capability presence does not establish persisted schema readiness. Incremental event
acknowledgement, durable Product-to-SalesChannel links, owner evidence, and consumer
cutover remain open. See [`m7-product-source.md`](./m7-product-source.md),
[`m7-product-variant-source.md`](./m7-product-variant-source.md),
[`m7-product-graph-source.md`](./m7-product-graph-source.md),
[`m7-product-tombstone-source.md`](./m7-product-tombstone-source.md), and
[`m7-sales-channel-source.md`](./m7-sales-channel-source.md).

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
cargo test -p rustok-index postgres_compiler_tests -- --nocapture
cargo test -p rustok-index postgres_query_result_tests -- --nocapture
cargo test -p rustok-index source_registry --lib -- --nocapture
cargo test -p rustok-index source_event_id --lib -- --nocapture
cargo test -p rustok-index source_replay --lib -- --nocapture
cargo test -p rustok-index source_replay_job --lib -- --nocapture
cargo test -p rustok-index source_replay_runner --lib -- --nocapture
cargo test -p rustok-index source_reconciliation --lib -- --nocapture
cargo test -p rustok-index source_factory --lib -- --nocapture
cargo test -p rustok-index replay_runtime --lib -- --nocapture
cargo test -p rustok-distribution --all-targets --features mod-product -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture
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

Targeted M3/M4/M5/M6/M7 guards:

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
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-index-query-planner.mjs
node scripts/verify/verify-index-postgres-query-compiler.mjs
node scripts/verify/verify-index-query-result-decoder.mjs
node scripts/verify/verify-index-many-link-filtering.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-replay-multipage-runner.mjs
node scripts/verify/verify-index-source-reconciliation.mjs
node scripts/verify/verify-index-replay-runtime-composition.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-variant-source.mjs
node scripts/verify/verify-index-product-graph-source.mjs
node scripts/verify/verify-index-product-tombstone-source.mjs
node scripts/verify/verify-index-sales-channel-source.mjs
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
  owner orchestration, read-only retained bundle review, admitted archive manifest
  generation, saved-manifest verification receipts, and recursive filesystem snapshot
  enforcement.
- 2026-07-29: completed deterministic typed M4 planning, controlled PostgreSQL query
  compilation, many-link filtering/projection, strict result decoding, and the
  PostgreSQL query port source boundary.
- 2026-07-30: completed retained v4 plan/SQL snapshots, explicit many-link aggregate
  ordering, source-owned schema publication, shared query runtime, the bounded source
  registry, one-page replay, durable replay jobs/checkpoints, bounded multi-page
  execution, cancellation, runtime/operator composition, and the first Product source.
- 2026-07-31: rechecked merged M7 work and actualized this plan. ProductVariant and
  SalesChannel sources, Product v2 graph links, retained Product/translation/
  ProductVariant hard deletes, and bounded multi-pass reconciliation were already in
  `main` but had stale open checklist entries.
- 2026-07-31: added retained SalesChannel hard-delete identities, strict identity-reuse
  fencing, live/tombstone conflict rejection, and generic replay deletes through the
  existing stable SalesChannel source. No schema fingerprint, source name, cursor shape,
  or event domain changed.
- Repository test/fixture suites, verifiers, PostgreSQL execution, in-page interruption,
  automatic retry/backoff, dead-letter and host scheduling, graceful task shutdown,
  authorized command transports, incremental event acknowledgement, persisted schema
  readiness, durable Product-to-SalesChannel relations, retained freshness/recovery and
  equivalence evidence, one admitted PostgreSQL/reference equivalence bundle, and one
  real full PostgreSQL partition packet remain for the owner to execute and admit before
  authoritative consumer or production partition cutover.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `Index ownership, migrations, mutation/version ordering, query execution, partition evidence guards, source replay ownership, replay checkpoint ordering, replay job attempt fencing, bounded multi-page replay, bounded reconciliation, cancellation ordering, host replay composition, operator authorization, Product/ProductVariant/SalesChannel source composition, Product graph links, retained delete identities, stable source identity, and source/search/server boundaries`
- Findings: `P0=0, P1=2, P2=0, P3=1`
- Fixed in this pass: `actualized stale M6/M7 plan state; preserved bounded replay and reconciliation non-claims; added Channel-owned retained hard-delete identities with monotonic reuse fencing; replayed SalesChannel deletes through the existing source with the same schema fingerprint, source name, cursor and event domain; added fail-closed live/tombstone identity checks, documentation, and a static repository guard`
- Remaining risks or blockers: `one retained admitted real PostgreSQL partition packet is absent; live PostgreSQL/reference query equivalence remains open; in-page interruption, automatic retry/backoff, dead-letter scheduling, host scheduling, graceful task shutdown, authorized command transports, locale/partition checkpoint dimensions, mutation-source event acknowledgement, persisted per-tenant schema readiness, durable Product-to-SalesChannel relations, tombstone purge admission, and additional production source adapters remain absent; no retained per-tenant freshness/watermark, lag, negative-result safety, outage/restart/backlog catch-up, delete/recreate, cancellation, authorization, replay, reconciliation, or repair evidence exists; consumer and production partition cutover remain forbidden`
- Evidence: `PR #2604 added the first Product source; PR #2611 added ProductVariant; PR #2616 added SalesChannel; PR #2623 added Product graph v2 and Product-to-ProductVariant links; PR #2628 added retained Product/translation/ProductVariant deletes; PR #2632 added bounded source reconciliation. This SalesChannel tombstone slice is source-ready without implementation-agent test, verifier, Cargo, CI, or PostgreSQL execution.`
- Next action: `define the source-versioned incremental mutation event and acknowledgement contract, then persist/apply exact per-tenant M7 schemas and add durable Product-to-SalesChannel relations; execute retained delete/recreate, replay/reconciliation, freshness/outage/recovery, live query equivalence, and partition evidence before authoritative cutover`
- Resume command: `cargo fmt --all -- --check && cargo check -p rustok-channel --all-targets && cargo check -p rustok-index --all-targets && cargo check -p rustok-distribution --all-targets --features mod-product && cargo check -p rustok-server --all-targets && node scripts/verify/verify-index-sales-channel-source.mjs && node scripts/verify/verify-index-source-reconciliation.mjs && node scripts/verify/verify-index-product-tombstone-source.mjs && node scripts/verify/verify-index-query-contract.mjs && node scripts/verify/index-storage-tooling.mjs contract && node scripts/verify/index-storage-tooling.mjs fixtures`
