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
Benchmark/evidence code remains outside production migrations and runtime composition.
Partition cutover remains forbidden until one retained real PostgreSQL packet satisfies
the explicit admission policy.

The repository owner performs test, benchmark, and evidence execution. Commits and pull
requests record checks and PostgreSQL runs that were not executed.

## Current state

- Rewrite status: `in_progress`
- Current milestone: `M5/M6 - bounded source replay owner contract (source complete; durable worker and retained evidence open)`
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
- M5/M6 bounded source replay contract: `source_complete_worker_pending`

Production persistence, typed planning/compilation/decoding, explicit many-link MIN/MAX
aggregate ordering with the exact Decimal string wire, source-owned schema publication,
shared query-runtime composition, and the default-off Social Graph privacy parity shadow
are source complete. The new neutral replay boundary provides one schema-owned source,
bounded scan/load requests, opaque bounded cursors, result-scope validation, and retryable
versus permanent failure classification. One retained admitted partition packet, live
PostgreSQL/reference equivalence, durable ingestion/rebuild workers, freshness and recovery
evidence, authoritative consumer cutover, and production partition lifecycle remain open.

## Ownership and architecture

Index owns schema/link/source registration, generic records and mutations, ingestion,
inbox deduplication, PostgreSQL storage, query validation/planning/compilation, filtering,
projection, sorting, counting, pagination, rebuild, checkpointing, reconciliation, drift
repair, distributed coordination, and operator diagnostics.

Source modules own normalized domain data, schema declarations, conversion to generic Index
records/mutations, bounded `IndexSource::scan` and targeted `load` adapters, and source
ordering/version information.

```text
source modules
    -> IndexSchemaSourceCatalog / IndexSourceCatalog / IndexMutation
    -> ingestion and rebuild engines
    -> PostgreSQL index storage
    -> schema/link registry and query planner
    -> SQL compiler
    -> IndexQueryPort
    -> server, storefront, admin, and rustok-search
```

Use workspace `sea-orm`/SeaQuery for PostgreSQL, `tokio`/`futures-util` for bounded async
work, `serde`/`postcard` for contracts, `thiserror` for typed failures,
`tracing`/telemetry/prometheus for observability, `petgraph` for deterministic graph
resolution, ICU4X for locale canonicalization, and `sha2` for schema, cursor, plan, and
retained-evidence identities.

Forbidden in Index core: source-domain dependencies, ranking/search libraries, a second
database stack, unvalidated JSON-only public queries, unbounded rebuild ID collection,
direct source-table reads, source-owned writes to Index tables, and destructive partition
cutover without retained evidence and rollback proof.

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
- [x] Validate records, mutations, queries, types, operators, cardinality, tenant/locale
      scope, and complexity.
- [x] Add checksummed query-scoped keyset cursors.
- [x] Add a test-only reference engine and property invariants.

### M2 - PostgreSQL storage benchmark

- [x] Define the benchmark contract and keep candidate DDL outside production migrations.
- [x] Add deterministic `smoke`, `100k`, and `1m` dataset presets.
- [x] Prototype JSONB entity rows plus typed expression/GIN indexes.
- [x] Compare JSONB, typed EAV, and specialized hot projection using equal result digests,
      cardinality, planner/session metadata, WAL, buffers, relation size, churn, and
      VACUUM evidence.
- [x] Run and archive replacement 100k Product-locale row evidence.
- [x] Run and archive replacement 1m Product-locale row evidence.
- [x] Record the selected model and rejected alternatives in an ADR.
- [x] Delete benchmark prototypes that are not selected.

M2 is complete. The JSONB harness is selected-layout regression evidence, not production
persistence. Canonical production tables remain unpartitioned until separate shadow
evidence is retained and admitted.

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
- [ ] Cover migration-from-zero, stale mutation, redelivery, rollback, concurrency, and
      tenant/locale isolation in PostgreSQL.

#### Retained repository contract wording

- [x] Add locking/leases for schema application.
- [x] Add secondary-index planning and lifecycle management.
- [x] Add tenant/schema/entity/locale keys and source-version guards.
- [x] Add immutable partition evidence manifest, measured packet validator, and
      owner-operated runbook.
- [x] Add exact-byte raw-artifact capture assembly with bundle confinement and no-clobber
      packet publication.
- [x] Add read-only retained bundle review, admitted archive manifest, and saved-manifest
      verification receipt.
- [x] Bind retained verification to an exact recursive filesystem snapshot and refuse
      post-inspection drift.
- [ ] Execute retained PostgreSQL partition baseline/shadow evidence.
- [ ] Execute retained PostgreSQL query, mutation, maintenance, and cutover evidence.
- [ ] Execute retained PostgreSQL mutation, maintenance, and cutover evidence.

Partition admission remains fail-closed across tenant-predicate coverage, query/mutation
latency and plan stability, WAL amplification, skew, and cutover lock evidence. Admitted
plans emit shadow-only DDL. They never rename, drop, or alter production relations.

#### Completed M3 slices

1. Canonical migrations register the seven module-owned tables with complete tenant,
   schema, entity, locale, and full-range non-negative source-version identity.
2. `PostgresMutationStore` validates deliveries, claims the composite inbox key,
   serializes the entity key, and applies entity/tombstone/link replacement atomically.
3. `PostgresSchemaLeaseStore` provides durable schema-application exclusion, reclaim,
   heartbeat, terminal completion, and attempt fencing.
4. `SecondaryIndexPlan` and `PostgresSecondaryIndexManager` derive typed/GIN indexes,
   coordinate concurrent ensure/reindex/retire work, and verify catalog readiness.
5. `PartitionAdmissionPolicy` and `PartitionShadowPlan` implement measured, fail-closed
   admission and deterministic shadow-only hash partition DDL.
6. Immutable manifest preparation and packet validation bind commit, image, modulus,
   repetitions, thresholds, evidence identity, raw hashes, and calculated reasons.
7. Exact-byte capture assembly reads six unique confined raw files once, validates the
   packet, and refuses overwrite or aliases.
8. Snapshot/query/mutation/maintenance/cutover runners retain parity, plans, latency,
   WAL, skew, maintenance, and rollback-only lock evidence without touching canonical DDL.
9. Full-capture orchestration, retained review, admitted-only archive manifests, and
   saved-manifest receipts fail closed on identity, inventory, metadata, alias, or byte drift.

### M4 - Query engine v1

- [x] Produce deterministic executable plans from validated queries.
- [x] Resolve explicit link paths and assign stable aliases.
- [x] Capture typed referenced-field contracts in executable plans.
- [x] Compile plans through SeaQuery or controlled SQL.
- [x] Support root and one-cardinality-link projection, filtering, sorting, exact count,
      and keyset pagination.
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

Many-link ordering is admitted only through caller-explicit `min_asc`, `min_desc`,
`max_asc`, and `max_desc`. Integer, string, timestamp, and Decimal terminals retain typed
PostgreSQL aggregation and ordering; Decimal encodes only the hidden aggregate order value
through the exact tagged string wire. UUID aggregate ordering, implicit first-row semantics,
and aggregate cursor continuation remain fail closed. Aggregate ordering is
bounded-offset-only and does not change root cardinality or exact count.

Source-owned schema publication, shared query-runtime composition, retained plan/SQL
snapshots, and the first default-off Social Graph privacy parity shadow are source complete.
The shadow is non-authoritative, records bounded parity observations, uses only the caller's
remaining deadline budget, and always returns the owner result. Live metrics/evidence
execution and authoritative consumer cutover remain open.

### M5 - Incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [ ] Add a mutation-source event registry and broker acknowledgement orchestration.
- [x] Add inbox deduplication and monotonic source versions.
- [ ] Add batch transactions, retry policy, bounded backoff, dead-letter state, and lag
      metrics around the source contracts.
- [ ] Protect the complete event-to-ack path against out-of-order update/delete delivery.
- [ ] Cover crash between commit and acknowledgement.

The source registry fixes one source owner for every exact schema and the complete schema
identity across versions. It materializes only when every source schema is published by the
same module owner. Source failures expose only bounded machine-readable codes classified as
retryable or permanent; raw source or storage causes stay in owner logs.

### M6 - Rebuild and reconciliation

- [x] Add cursor-based `IndexSource::scan` and targeted `load` contracts.
- [x] Bound cursor bytes, scan pages, targeted key counts, tenant/schema scope, uniqueness,
      and continuation progress; never collect all IDs first.
- [ ] Add durable jobs, checkpoints, leases, heartbeat, and ownership.
- [ ] Add a bounded worker that applies pages through `PostgresMutationStore` and commits
      checkpoints only after durable mutation results.
- [ ] Add cancellation, resume, dry-run, targeted/full/shadow rebuild.
- [ ] Add reconciliation and drift repair.
- [ ] Cover crash, lease expiry, restart, cancellation, and incremental/full equivalence.

No durable job runner, checkpoint writer, scheduler, or production replay command is claimed
by the source-complete contract. The existing `index_jobs` and `index_checkpoints` tables are
reserved persistence foundations until a fenced worker owns their state transitions.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [ ] Register owner-published schemas, links, and bounded source adapters.
- [ ] Implement incremental mutations and rebuild sources.
- [ ] Support tenant, locale, status, projection, link filters, sorting, and cursor pagination.
- [ ] Move one Storefront query to Index.
- [ ] Prove no source-module filtering fan-out.

### M8 - Commerce scale slice

- [ ] Add Pricing, Inventory, Category, Collection, Tags, Region/Currency, and Marketplace
      Seller schemas/sources.
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

- [ ] Expose schema, partition, lag, inbox, failure, rebuild, drift, and query diagnostics.
- [ ] Add rebuild/cancel/retry commands.
- [ ] Publish new FBA contracts and runtime evidence.
- [ ] Migrate consumers and delete final compatibility code.
- [ ] Promote FBA only after compiled/live evidence.

## Verification handoff

The owner runs formatting, Cargo checks/tests, PostgreSQL fixtures, evidence capture,
admission, and CI. Targeted source guards include:

```bash
node scripts/verify/verify-index-fba.mjs
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
node scripts/verify/verify-index-query-snapshots.mjs
node scripts/verify/verify-index-many-link-aggregate-ordering.mjs
node scripts/verify/verify-index-decimal-aggregate-wire.mjs
node scripts/verify/verify-index-source-schema-registry.mjs
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-social-graph-privacy-consumer.mjs
cargo xtask module validate index
```

## Progress log

- 2026-07-23 through 2026-07-28: completed the destructive reset, M1 domain core,
  selected-layout M2 evidence, and source-complete M3 storage/evidence tooling.
- 2026-07-29: completed typed M4 planning, controlled SQL, filters, projection,
  pagination, strict decoding, PostgreSQL query-port source, retained v4 snapshots,
  source-owned schema publication, shared query-runtime composition, and the first
  default-off owner-authoritative Social Graph privacy parity shadow.
- 2026-07-30: completed explicit many-link MIN/MAX aggregate ordering for integer,
  string, timestamp, and Decimal terminals; locked Decimal aggregate order values to the
  exact tagged string wire; retained metrics/two-snapshot shadow evidence tooling; and
  bounded each non-authoritative comparison by the caller's remaining deadline.
- 2026-07-30: added the neutral bounded replay source registry, opaque JSON cursor and
  scan/load contracts, exact schema-owner materialization, tenant/schema/result bounds,
  continuation-progress checks, and retryable/permanent source failure classification.
- Repository test/fixture suites, the durable replay worker, live freshness/recovery
  evidence, authoritative cutover, PostgreSQL/reference equivalence, and one real full
  PostgreSQL partition packet remain owner-executed gates.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `Index ownership, migrations, mutation/version ordering, query execution, partition evidence guards, Social Graph consumer authority, privacy shadow deadline behavior, source replay ownership, and source/search/server boundaries`
- Findings: `P0=0, P1=2, P2=0, P3=1`
- Fixed in this pass: `bounded the non-authoritative Social Graph privacy comparison by the remaining caller deadline; repaired stale Index scale/FBA guards; added a canonical bounded source replay/load registry with strict schema ownership, request/result bounds, continuation progress, and retryable/permanent failure classification`
- Remaining risks or blockers: `one retained admitted real PostgreSQL partition packet is absent; live PostgreSQL/reference query equivalence remains open; the new source contract has no fenced durable job/checkpoint worker; no retained per-tenant freshness/watermark, lag, negative-result safety, outage/restart/backlog catch-up, or replay-repair evidence exists; consumer and production partition cutover remain forbidden`
- Evidence: `PR #2544 merged as b647a5a17f27b34a07c20cd57525ed1806994c49; same-SHA Index Storage Scale Run Contract and full Scale Evidence contract passed JavaScript syntax, workflow contracts, repository contracts, direct validator arguments and fixture suites; source replay contract added in the current draft PR without implementation-agent test execution; general CI/Hardening received no jobs; Rust-hosted smoke previously stopped before compilation because Cargo.lock required an Athanor update under --locked`
- Next action: `implement fenced durable rebuild jobs/checkpoints and the first Product source adapter, then execute retained PostgreSQL packet, live query equivalence, and per-tenant freshness/outage/recovery evidence before reconsidering authoritative consumers or partition lifecycle`
- Resume command: `cargo fmt --all -- --check && cargo check -p rustok-index --all-targets && node scripts/verify/verify-index-query-contract.mjs && node scripts/verify/index-storage-tooling.mjs contract && node scripts/verify/index-storage-tooling.mjs fixtures`