# rustok-index

## Purpose

`rustok-index` is RusToK's cross-module relational Index Engine. Source modules
publish generic schemas, records, mutations, links, and bounded replay sources;
Index materializes them into optimized storage and executes filtering, projection,
sorting, counting, and pagination without runtime fan-out to source modules.

Backward compatibility with the rejected source-specific implementation is not
a rewrite goal.

---

## Architectural Comparison Matrix

| Indexing Approach | Write Overhead | Cross-Module Filtering | Zero N+1 Queries | Consistency Model | Infrastructure Complexity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Traditional SQL JOINs** | Low | Slow (Multi-table JOIN bottlenecks) | No (N+1 HTTP calls across microservices) | Immediate | Single DB |
| **EAV Tables (Magento / Legacy CMS)** | High DDL & Row Bloat | Complex self-JOINs & lock contention | No | Immediate | Single DB |
| **External Search Sync (Elasticsearch / Algolia)** | High (Outbox / Event Relay) | Fast | Yes | Eventual (Lag & drift risks) | Heavy JVM/Cloud cluster |
| **`rustok-index` (JSONB + Keyset Engine)** | **Low (Transactional Outbox Inbox)** | **Ultra-Fast (Derived B-Tree / GIN Indexes)** | **Yes (Single REPEATABLE READ query)** | **Immediate (Transactional Outbox)** | **Pure Rust + PostgreSQL (Zero external dependencies)** |

---

## Responsibilities

- Own the generic schema, link, and replay-source registries.
- Own incremental ingestion, deduplication, rebuild, reconciliation, and drift
  control.
- Own PostgreSQL index storage and distributed coordination.
- Validate and plan cross-module queries.
- Compile projection, filtering, sorting, count, and pagination to Index storage
  queries.
- Execute compiled queries through Index-owned PostgreSQL ports.
- Publish stable query, source, rebuild, replay-runtime, and operator contracts.
- Keep product-facing relevance and ranking in `rustok-search`.

## Interactions

- Source modules register generic schemas, mutations, links, and bounded replay
  sources without exposing their tables to Index.
- `rustok-distribution` assembles the selected source registrations into the
  immutable schema and replay-source catalogs.
- `apps/server` composes the guarded query, replay, and drift-diagnosis runtimes
  with its database and request-bound authorization context.
- `rustok-search` consumes Index-owned records and query contracts for search
  projection, while retaining ranking and search-specific behavior.

## Boundaries

- Index core must not depend on Product, Content, Flex, Pricing, Inventory, or
  other source-domain crates.
- Source modules own conversion from domain state/events into generic records,
  mutations, and bounded replay/load adapters.
- Index must not read source-module tables directly.
- Source modules do not write Index storage, job, or checkpoint tables.
- `rustok-search` owns ranking, typo tolerance, autocomplete, synonyms, search
  UX, and external search-engine connectors.
- The selected JSONB regression DDL lives under `ops/benches`; it is not a
  production migration or runtime storage contract. Historical three-candidate
  evidence is archived under `docs/evidence/`.
- Replay runtime composition does not create a scheduler, background task, or
  transport authorization surface.

## Rewrite status

- Current milestone: `M6 - replay runtime host composition and operator guard`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: complete
- M1 generic domain/application core: complete
- M2 PostgreSQL storage benchmark: complete
- M2 accepted storage model: JSONB
- M2 rejected prototype cleanup: complete
- M3 storage-schema foundation: complete
- M3 atomic mutation persistence: complete
- M3 schema-application leases: complete
- M3 secondary-index lifecycle: complete
- M3 partition admission and shadow planning: complete
- M3 partition evidence capture and packet assembly: complete
- M3 retained packet owner orchestration: complete
- M3 retained bundle review and archive verification: complete
- M4 deterministic query planning and controlled SQL compilation: complete
- M4 root/one and many-link query result semantics: complete
- M4 explicit many-link aggregate ordering and Decimal tagged wire: source complete
- M4 PostgreSQL query port and row adapter: source complete
- M4 source-owned registry and server query-runtime composition: source complete
- M5/M6 bounded source registry and replay/load contracts: source complete
- M6 one-page replay and durable checkpoint progression: source complete
- M6 replay job leases and checkpoint attempt fencing: source complete, owner execution pending
- M6 bounded multi-page replay and cancellation: source complete, owner execution pending
- M6 replay runtime host composition and operator guard: source complete
- Real retained PostgreSQL packet execution: open
- In-page interruption, retry/dead-letter scheduling, host scheduler, command transports,
  and Product adapters: open
- Query-port authorization/consumer cutover and live equivalence evidence: open

All legacy ports, adapters, source indexers, projections, migrations, runtime
configuration, scheduler, and errors from the rejected source-specific design remain
deleted. M3 registers the canonical production schema, publishes an Index-owned
transactional mutation adapter, owns durable schema-application leases, manages
deterministic schema-derived secondary indexes, and rejects partition rollout until
measured shadow evidence passes an explicit policy. Owner-operated tooling captures
and validates the nine-file retained bundle, renders a read-only review, emits an
admitted-only archive manifest outside the bundle, and verifies the saved manifest
against an exact recursive filesystem snapshot.

M4 provides deterministic typed planning, controlled PostgreSQL compilation,
correlated many-link filtering, nested many-link projection aggregation, explicit bounded
`MIN` / `MAX` many-link ordering for integer, Decimal, string, and timestamp, strict result
decoding, lookahead pagination, exact count, scoped cursors for ordinary order expressions,
an Index-owned PostgreSQL execution port, a source-owned immutable schema catalog, and a
server-owned neutral query runtime. Decimal aggregate ordering remains numeric while its
hidden tagged value uses an exact JSON string. Aggregate cursor continuation,
PostgreSQL/reference aggregate evidence, storefront/admin/search authorization and consumer
cutover, and production partition cutover remain open.

M5/M6 adds a neutral `IndexSource` boundary with bounded opaque cursor scans and targeted
loads, exact schema/source ownership, page and key limits, continuation progress checks,
and bounded retryable/permanent source failures. `IndexReplayWorker::run_next_page`
validates the complete page event-identity set before any write, applies mutations through
`PostgresMutationStore`, and advances the durable rebuild checkpoint only after every
mutation result is committed. Stable event UUID delivery IDs make retry after checkpoint
failure idempotent through the existing inbox contract.

`PostgresIndexReplayJobStore` owns one schema-scoped `rebuild` job per tenant/source/schema.
It validates the exact `index_replay_job_v1` request and active persisted schema, serializes
claims with a PostgreSQL advisory lock, heartbeats an unexpired lease, reclaims expired
work with an incremented attempt fence, and rejects stale terminal updates.
`PostgresIndexReplayCheckpointStore` is constructed from the acquired
`IndexReplayJobLease`; every checkpoint read or write locks and validates the exact
`(job_id, worker_id, attempt_count)` first. A stale worker may finish an already-started
idempotent mutation transaction, but it cannot advance the durable cursor. Successful
job completion requires an active lease and the exact durable rebuild checkpoint with a
JSON null cursor.

`PostgresIndexReplayRunner` resolves source ownership from `SharedIndexSourceRegistry`,
processes at most the validated 1024-page invocation budget, heartbeats only between
pages, yields unfinished work to an immediately claimable pending attempt, and observes
durable cancellation before and after page execution. A cancellation committed first
cannot be overwritten by success, failure, or yield.

The server freezes `SharedIndexSourceRegistry` after all selected modules register sources
and calls the Index-owned `materialize_postgres_index_replay_runtime`. This binds the exact
immutable schema and source registries to the host database and publishes
`SharedIndexReplayRuntime` through `ModuleRuntimeExtensions`. Composition performs no SQL
and starts no task.

The raw runtime is then wrapped by the server-owned `IndexReplayOperatorRuntime`. It
requires a request-bound effective permission snapshot for an exact non-nil tenant/actor,
rejects cross-tenant invocation, requires `modules:manage`, and exposes only bounded
`run` and `request_cancel`. Future GraphQL, HTTP, CLI, or admin transports must consume this
guarded operator runtime rather than the raw infrastructure capability.

Runtime presence does not claim persisted tenant schema readiness, source availability,
automatic retry, scheduler ownership, graceful shutdown, command authorization, successful
replay, or retained PostgreSQL evidence. In-page interruption, retry/backoff/dead-letter
policy, a global host scheduler and stop-handle ownership, locale/partition replay
dimensions, production source adapters, and retained multi-instance evidence remain open.

The module-owned migration source creates:

- `index_schemas` for versioned owner-published schema contracts;
- `index_entities` for the tenant-leading JSONB entity envelope and full-range `DECIMAL(20,0)` source version;
- `index_links` for independently relational ordered links;
- `index_inbox` for durable delivery deduplication and leases;
- `index_checkpoints` for ingestion/rebuild cursors;
- `index_jobs` for schema, index, rebuild, reconciliation, and consistency work;
- `index_consistency_findings` for durable drift diagnostics.

The `PostgresMutationStore` applies one `MutationDelivery` transactionally. It
validates the mutation through `SchemaRegistry`, reserves the tenant-scoped inbox
identity, rejects delivery-ID payload reuse, takes a transaction-scoped advisory
lock on the complete entity key, reads the current source version, and either
terminally ignores a stale delivery or replaces the live JSONB fields and ordered
links with the incoming state. Deletes write a tombstone. The inbox row is
completed in the same commit. Exact redelivery returns
`MutationApplyOutcome::Duplicate`; a failed entity/link write rolls back the inbox
claim. SQLite support exists only for contract fixtures and rejects source versions
above its signed integer range; PostgreSQL preserves the full domain `u64` range
through `DECIMAL(20,0)`.

The `PostgresSchemaLeaseStore` coordinates one `schema_apply` job per exact
tenant/module/entity/schema-version scope. Acquisition is serialized with a
transaction-scoped PostgreSQL advisory lock, verifies the persisted schema and
fingerprint, returns `Busy` while another non-expired owner holds the lease, and
returns `AlreadyApplied` after terminal success. Expired work is reclaimed with an
incremented attempt fence; heartbeat, success, and failure require the exact job,
worker, attempt, running state, and unexpired lease so an old owner cannot commit
after takeover. SQLite support remains contract-test-only.

`SecondaryIndexPlan` derives one deterministic index specification for every
filterable or sortable field. Scalar fields use typed partial B-tree expressions;
filterable `many` fields use field-local JSONB containment GIN. Expressions follow
the production tagged `IndexValue` payload contract through each field's `value`
member. Stable names bind tenant, schema reference, schema fingerprint, field type,
cardinality, and index kind. `PostgresSecondaryIndexManager` coordinates ensure,
reindex, and retirement through fenced `secondary_index` jobs, executes PostgreSQL
`CONCURRENTLY` DDL, records owner definition comments, and verifies `indisready`
and `indisvalid` before completion. Retire remains available for retired schemas;
SQLite is contract-test-only.

`evaluate_partition_admission` keeps the canonical tables unpartitioned unless one
explicit policy is satisfied by one exact SHA-256 evidence packet. Admission checks
minimum measured size/row/tenant scale, tenant-predicate coverage, entity and link
digest parity, shadow catch-up, foreign-key validation, orphan-link absence, query
plan stability, p95 query/mutation regressions, WAL amplification, partition-size
skew, and cutover-lock duration. An admitted `PartitionShadowPlan` derives stable
hash-partition parent/child names and PostgreSQL bootstrap statements for shadow
relations only. It never emits production table rename, drop, or cutover SQL.
Tenant-hash modulus must be a power of two from 2 through 128. Actual copy,
constraint/index attachment, dual-write/replay, cutover, rollback, and durable
global operation ownership remain later work and require retained PostgreSQL
evidence.

The retained evidence boundary is owner-operated and read-only after capture. Review
recalculates packet assembly and admission from exactly nine authoritative files.
Archive verification rereads files and the saved external manifest through stable
descriptors, checks filesystem identity and metadata fingerprints, verifies the
exact recursive directory inventory, and fails closed on aliases, replacement,
metadata drift, inventory drift, or byte drift. Public manifest and receipt schemas
remain stable, and every successful receipt keeps
`production_lifecycle_authorized: false`.

`IndexQueryPort` is the transport-neutral query execution boundary.
`PostgresIndexQueryPort` owns a PostgreSQL connection and immutable
`Arc<SchemaRegistry>`. Each call compiles through the registry, starts one read-only
repeatable-read transaction, verifies every root/source/target schema against the
query tenant's exact active `index_schemas` fingerprint and JSON contract, executes
the page and optional exact-count statements in the same snapshot, maps only
compiler-declared UUID/JSONB/bigint aliases, and delegates semantic validation and
cursor construction to `decode_postgres_query_page`. Authentication and transport
policy remain caller responsibilities.

`IndexSchemaSourceCatalog` collects owner-published generic contracts during module
registration and fixes one owner for each complete schema identity across versions.
`rustok-distribution` materializes all entries through one atomic batch and publishes
`SharedIndexSchemaRegistry` only for a non-empty catalog. The server then calls the
Index-owned `materialize_postgres_index_query_runtime`, which binds that exact immutable
registry to the host database and publishes `SharedIndexQueryRuntime` through
`ModuleRuntimeExtensions`. Composition performs no SQL and does not claim tenant schema
readiness; execution still fails closed through the query-port preflight.

`IndexSourceCatalog` separately fixes one replay source for each exact schema and the
complete schema identity across versions. Materialization requires the corresponding
owner-published schema and exact owner match. The application replay boundary remains
database independent; PostgreSQL mutation, job, checkpoint, runner, and host runtime
adapters are composed outside it.

## Current entry points

- `IndexModule`
- `rustok_index::domain::*`
- `rustok_index::application::*`
- `rustok_index::migrations::*`
- `PostgresMutationStore`, `MutationDelivery`, and `MutationApplyOutcome`
- `PostgresSchemaLeaseStore`, `SchemaApplicationLeaseRequest`,
  `SchemaApplicationLease`, and `SchemaLeaseAcquireOutcome`
- `PostgresIndexReplayJobStore`, `IndexReplayJobLeaseRequest`,
  `IndexReplayJobLease`, and `IndexReplayJobAcquireOutcome`
- `PostgresIndexReplayCheckpointStore`, `IndexReplayWorker`,
  `IndexReplayCheckpoint`, and `IndexReplayPageOutcome`
- `PostgresIndexReplayRunner`, `IndexReplayRunRequest`, `IndexReplayRunOutcome`,
  `IndexReplayCancelOutcome`, and `IndexReplayRunError`
- `SharedIndexReplayRuntime`, `materialize_postgres_index_replay_runtime`, and
  `IndexReplayRuntimeCompositionError`
- server-owned `IndexReplayOperatorRuntime`, `IndexReplayOperatorContext`, and
  `IndexReplayOperatorError`
- `IndexSource`, `IndexSourceCatalog`, `SharedIndexSourceRegistry`,
  `IndexSourceScanRequest`, and `IndexSourceLoadRequest`
- `SecondaryIndexPlan`, `SecondaryIndexSpec`, `SecondaryIndexRequest`,
  `SecondaryIndexLease`, and `PostgresSecondaryIndexManager`
- `PartitionAdmissionPolicy`, `PartitionEvidence`, `PartitionAdmissionOutcome`,
  `PartitionShadowPlan`, and `evaluate_partition_admission`
- `SchemaRegistry`, `IndexSchema`, `IndexRecord`, and `IndexMutation`
- `IndexSchemaSourceCatalog`, `SharedIndexSchemaRegistry`, and
  `register_index_schema_source`
- `IndexQuery`, `IndexQueryScope`, `FilterExpr`, and typed `FieldPath`
- `ExecutableQueryPlan`, `CompiledPostgresQuery`, and `CompiledPostgresPageQuery`
- `IndexQueryPort`, `PostgresIndexQueryPort`, `SharedIndexQueryRuntime`,
  `materialize_postgres_index_query_runtime`, `IndexQueryExecutionError`, and
  `IndexQueryPage`
- `CursorCodec`, `IndexCursor`, and query-scope cursor validation

## Implemented invariants

- bounded lowercase identifiers;
- ICU4X syntax and CLDR alias locale canonicalization;
- stable order-independent schema fingerprints;
- atomic versioned schema registration;
- deterministic link-path resolution;
- tenant/locale-scoped records and queries;
- registry-backed type, cardinality, field, link, and operator validation;
- bounded query complexity and pagination;
- plain `asc` / `desc` remains ambiguous through `many`; explicit bounded `min` / `max`
  supports integer, Decimal, string, and timestamp;
- Decimal many-order `MIN` / `MAX` and `ORDER BY` use `numeric`, while hidden tagged JSON uses
  an exact string from `numeric::text` without float conversion;
- checksummed keyset cursors bound to tenant, schema, fingerprint, locale, and
  order shape;
- reference mutation/query engine and property-based invariants for future
  PostgreSQL equivalence tests;
- atomic tenant-scoped inbox/entity/link mutation persistence with monotonic
  source-version and tombstone admission;
- durable schema-application exclusion, expiry reclaim, heartbeat, terminal
  completion, and attempt fencing;
- deterministic tenant/schema/fingerprint-bound secondary-index names,
  tagged-payload expressions, concurrent lifecycle, owner verification, catalog
  readiness checks, and operation fencing;
- fail-closed partition admission, exact evidence identity, explicit regression
  limits, deterministic tenant-hash shadow names, and no destructive cutover SQL;
- exact-byte retained review and admitted archive verification bound to stable file
  and directory identities, metadata fingerprints, and recursive inventory without
  production lifecycle authorization;
- deterministic typed plans, controlled ordered binds, duplicate-free many-link
  `EXISTS` filtering, row-preserving nested many-link JSONB aggregation, strict
  decoded column contracts, exact count, lookahead, and scoped cursor construction;
- PostgreSQL-only query execution with exact persisted schema preflight, one
  read-only repeatable-read page/count snapshot, exhaustive bind conversion, and
  compiler-metadata-driven row mapping;
- deterministic source-schema and replay-source ownership, atomic registry
  materialization, bounded cursor/page/key contracts, and no false empty runtime;
- full-page event UUID validation before mutation persistence, stable replay delivery
  identity, mutation-before-checkpoint ordering, monotonic checkpoint watermarks, and
  duplicate-safe replay after checkpoint failure;
- durable schema-scoped rebuild exclusion, lease heartbeat, expired-attempt reclaim,
  attempt fencing, stale checkpoint-writer rejection, and null-cursor-gated success;
- bounded multi-page execution, immediate resume, durable cancellation, and
  cancellation-first terminal ordering;
- one Index-owned PostgreSQL query-runtime constructor and neutral capability transfer
  into `HostRuntimeContext`;
- one Index-owned replay-runtime constructor plus a server-owned request-bound
  `modules:manage` operator guard, with no startup SQL or background task.

## M2 benchmark

M2 compared JSONB, normalized typed EAV, and specialized hot projection with one
deterministic Product/Variant/SalesChannel dataset and identical read, mutation,
and maintenance workloads. The exact successful packets and comparison are
archived under `docs/evidence/2026-07-27-postgresql-storage/`.

The accepted ADR selects JSONB. The typed-EAV and hot-projection implementations
are removed; `ops/benches` retains only a JSONB selected-layout regression harness.
It verifies source/JSONB result parity, transactional mutation evidence, committed
churn, relation size, WAL, buffers, and ordinary `VACUUM (ANALYZE)` behavior. Its
DDL remains benchmark-only and must not be copied into production migrations.

## Docs

- [Module documentation](./docs/README.md)
- [Live implementation plan](./docs/implementation-plan.md)
- [M5/M6 bounded source replay contract](./docs/m5-m6-source-replay-contract.md)
- [M6 replay job lease and fencing boundary](./docs/m6-replay-job-leases.md)
- [M6 bounded multi-page replay runner](./docs/m6-bounded-multipage-runner.md)
- [M6 replay runtime host composition](./docs/m6-replay-runtime-composition.md)
- [M4 source-owned schema registry](./docs/m4-source-schema-registry.md)
- [M4 query runtime composition](./docs/m4-query-runtime-composition.md)
- [M4 PostgreSQL query port contract](./docs/m4-postgres-query-port.md)
- [M4 many-link aggregate ordering](./docs/m4-many-link-aggregate-ordering.md)
- [M4 Decimal aggregate order wire](./docs/m4-decimal-aggregate-order-wire.md)
- [M4 many-link projection contract](./docs/m4-many-link-projection.md)
- [M2 storage benchmark contract](./docs/storage-benchmark.md)
- [M2 replacement evidence runbook](./docs/storage-evidence-runbook.md)
- [M3 retained partition capture runbook](./docs/partition-full-capture.md)
- [Index Engine rewrite ADR](../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Accepted storage ADR](../../DECISIONS/2026-07-24-index-storage-layout.md)
- [Platform docs index](../../docs/index.md)
