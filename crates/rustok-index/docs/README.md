# Documentation `rustok-index`

`rustok-index` is the platform-owned cross-module relational Index Engine. Source
modules publish generic schemas, records, mutations, links, and bounded replay
sources; Index materializes them into PostgreSQL and serves structured cross-module
queries without runtime fan-out.

## Purpose

- publish canonical schema, mutation, query, source, replay, rebuild, and operator contracts;
- keep ingestion, storage, planning, rebuild, checkpoint, and consistency semantics
  inside the module;
- provide server, storefront, admin, and `rustok-search` with a stable substrate for
  cross-module filtering, projection, sorting, count, and pagination;
- scale reads and rebuilds independently from source-module query paths.

## Responsibility Zone

- versioned schema and link registry;
- source-schema and replay-source ownership registries;
- generic records and mutations;
- explicit tenant/locale query scope;
- registry-backed record and query validation;
- deterministic link graph and field paths;
- versioned keyset cursors;
- incremental ingestion and inbox deduplication;
- bounded source scans and targeted loads;
- one-page and bounded multi-page replay progression;
- durable schema-scoped replay jobs, lease/heartbeat, attempt fencing, and cancellation;
- host-published replay runtime and request-bound operator guard;
- PostgreSQL storage and distributed coordination;
- schema application and secondary-index lifecycle;
- measured partition admission and shadow planning;
- retained partition evidence preparation, snapshot/query/mutation/maintenance
  capture, assembly, review, archive verification, and validation;
- SQL planning, compilation, execution, and result decoding;
- rebuild, checkpointing, reconciliation, and drift repair;
- operator health, lag, failure, and rebuild controls.

## Excluded scope

- text relevance and ranking;
- typo tolerance, synonyms, autocomplete, and search UX;
- external search-engine connectors;
- source-module table reads from Index core;
- source-owned writes to Index storage, job, or checkpoint tables;
- source-specific Product, Content, or Flex logic in the engine;
- a production scheduler, automatic retry/dead-letter loop, or unbounded replay loop
  in the current M6 slice;
- transport command authorization beyond the server-owned operator capability;
- destructive partition cutover without retained PostgreSQL evidence.

## Integration

- source modules publish generic schemas and bounded replay/load adapters through
  `ModuleRuntimeExtensions`;
- source events become generic `IndexMutation` values with stable event UUID delivery
  identities;
- `IndexModule` contributes canonical production migrations through platform migration
  composition;
- `rustok-distribution` materializes the immutable shared schema registry;
- the server freezes the complete source registry, publishes query/replay capabilities,
  and wraps replay invocation in an exact request-bound operator guard;
- server, storefront, admin, and `rustok-search` consume stable Index ports rather than
  reading Index tables directly;
- benchmark DDL and evidence remain isolated under `ops/benches` and never become
  runtime migrations.

## Rewrite policy

Backward compatibility with the rejected implementation is not a goal. Conflicting
code is deleted instead of preserved through compatibility layers. M0 removed the
complete source-specific implementation and its migrations, contracts, runtime
scheduler, server wiring, and admin table reads.

## M1 generic core

M1 provides:

- bounded lowercase schema identifiers;
- ICU4X locale parsing and UTS #35/CLDR alias canonicalization;
- stable SHA-256 schema fingerprints;
- atomic versioned schema registration with idempotency and conflict detection;
- link target/type/cardinality validation;
- deterministic shortest-path graph resolution through `petgraph`;
- typed root and linked field paths;
- explicit tenant and locale query scope;
- select/filter/order/operator/type validation and bounded query complexity;
- checksummed postcard/Base64 keyset cursors bound to query scope and schema
  fingerprint;
- a test-only mutation/query reference engine and property invariants for later
  PostgreSQL equivalence testing.

## M2 storage benchmark

Benchmark code lives outside the production crate in `ops/benches/src/index_storage`.
Candidate DDL is not a production migration or runtime storage contract.

The read/query harness provides deterministic scale datasets, selected-layout read
workloads, cardinality checks, result-digest parity, load/size measurement, and full
JSON `EXPLAIN (ANALYZE, BUFFERS, WAL)` evidence. The transactional mutation harness
provides update/delete workloads, affected entity/link parity, rollback isolation, and
node-level WAL evidence. The persistent maintenance harness provides committed churn,
exact cardinality guards, schema-size/table-stat snapshots, and ordinary
`VACUUM (ANALYZE)` duration.

Replacement same-commit evidence selected JSONB over typed EAV and hot projection.
Rejected candidate implementations were deleted. The remaining JSONB path is a
selected-layout regression harness, not production persistence. Partitioning was not
part of M2 evidence, so canonical relations remain unpartitioned by default.

## M3 PostgreSQL storage engine

The module-owned migration source creates seven generic tables without source-domain
columns or benchmark schemas:

- `index_schemas` stores exact versioned schema JSON and fingerprints;
- `index_entities` stores the JSONB payload/tombstone envelope with complete tenant,
  module, entity, schema-version, entity-ID, and locale identity;
- `index_links` stores ordered independent links bound to the source entity's exact
  full-range `DECIMAL(20,0)` source version;
- `index_inbox` stores deduplication, mutation identity, processing leases, and terminal
  outcomes;
- `index_checkpoints` stores ingestion and rebuild cursors;
- `index_jobs` stores bounded durable schema/index/rebuild/reconciliation work;
- `index_consistency_findings` stores open/resolved drift findings.

`PostgresMutationStore` validates each `MutationDelivery`, claims the composite inbox
identity, serializes the complete entity key with a transaction-scoped advisory lock,
applies monotonic entity/tombstone and ordered-link replacement, and completes the
inbox in one transaction. Exact redelivery is idempotent, stale delivery is terminally
ignored, payload reuse fails closed, and write failure rolls back the inbox claim.

`PostgresSchemaLeaseStore` coordinates exact tenant/schema application through
`schema_apply` jobs. It verifies persisted active schema/fingerprint state, returns
`Busy` or terminal `AlreadyApplied`, reclaims expired work with incremented attempt
fencing, and requires the exact current worker/attempt for heartbeat and completion.

`SecondaryIndexPlan` derives deterministic indexes from the exact schema contract.
Scalar filterable/sortable fields use typed partial B-tree expressions. Filterable
`many` fields use field-local JSONB containment GIN. `PostgresSecondaryIndexManager`
coordinates ensure, reindex, and retirement through fenced `secondary_index` jobs and
verifies PostgreSQL catalog readiness before completion.

`evaluate_partition_admission` compares one unpartitioned baseline with one exact
SHA-256 identified tenant-hash shadow packet under an explicit policy. It checks scale,
tenant-predicate coverage, entity/link parity, catch-up, foreign keys, orphan links,
query-plan regressions, p95 query/mutation regressions, WAL amplification,
partition-size skew, and cutover-lock duration. An admitted plan emits shadow-only DDL
and cannot rename, drop, or alter production relations.

Owner-operated tooling captures and validates the retained partition bundle. Snapshot,
query, mutation/WAL, maintenance, and rollback-only cutover runners remain isolated
from production lifecycle code. Assembly and review bind exact bytes, canonical paths,
file identity, metadata fingerprints, and recursive directory inventory. Every archive
verification receipt keeps `production_lifecycle_authorized: false`.

Real PostgreSQL packet execution and admission remain owner-operated and open.

## M4 query engine

M4 provides:

- deterministic validated executable plans and stable relation aliases;
- controlled PostgreSQL SQL with ordered typed binds;
- root and one-link projection, filtering, ordering, exact count, keyset, and bounded
  offset pagination;
- correlated many-link `EXISTS` filtering without duplicate root rows;
- deterministic nested many-link JSONB projection aggregates;
- explicit bounded many-link `MIN` / `MAX` ordering for integer, Decimal, string, and
  timestamp terminals;
- exact Decimal tagged string transport without float conversion;
- strict compiler-metadata-driven row decoding, one-row lookahead, exact-count decoding,
  and scoped next-cursor construction;
- `PostgresIndexQueryPort`, which verifies every persisted tenant schema and executes
  page/count inside one read-only repeatable-read transaction;
- source-owned schema publication and one shared immutable query runtime.

Authentication and transport policy remain caller responsibilities. Retained live
PostgreSQL/reference equivalence, authoritative consumers, aggregate cursor
continuation, and production partition cutover remain open.

## M5/M6 source replay and rebuild ownership

`IndexSourceCatalog` fixes one bounded replay source for every exact schema and the
complete schema identity across versions. Materialization requires the corresponding
owner-published schema and exact owner match. `IndexSource::scan` and `load` keep the
application boundary database independent while bounding cursor bytes, page/key counts,
tenant/schema scope, returned identities, and continuation progress.

`IndexReplayWorker::run_next_page` executes exactly one page. It validates every
non-nil unique event UUID before the first mutation write, applies mutations sequentially
through `PostgresMutationStore`, and commits the next cursor only after all mutation
results are durable. Stable event UUIDs make retry after checkpoint failure idempotent
through `index_inbox`. Source-version watermarks cannot regress, and JSON null is the
completed cursor.

`PostgresIndexReplayJobStore` owns one exact tenant/source/schema `rebuild` job using the
`index_replay_job_v1` request contract. It:

- requires an active persisted schema;
- serializes acquisition with a PostgreSQL advisory lock;
- returns `Busy` for an active owner;
- heartbeats only the exact unexpired worker/attempt;
- reclaims an expired running attempt with incremented `attempt_count`;
- rejects stale heartbeat, failure, success, and cancellation publication;
- requires an exact durable null-cursor checkpoint before terminal success.

`PostgresIndexReplayCheckpointStore` is constructed from an acquired
`IndexReplayJobLease`. Every checkpoint read/write first locks and validates the exact
`(job_id, worker_id, attempt_count)`. Another tenant/source/schema is rejected before
the database transaction. After reclaim, the old worker cannot advance the durable
cursor.

`PostgresIndexReplayRunner` executes a validated page budget of at most 1024 pages,
resolves the exact source from `SharedIndexSourceRegistry`, heartbeats only between
pages, returns unfinished work to immediately claimable pending state, and aggregates
mutation outcomes. Durable cancellation terminalizes pending jobs immediately, marks
running jobs for observation before/after pages, survives reclaim, and wins over
success/failure/yield when committed first.

The server materializes `SharedIndexSourceRegistry` only after all selected modules
finish registration. `materialize_postgres_index_replay_runtime` then requires both
immutable source and schema registries and publishes `SharedIndexReplayRuntime`.
`IndexReplayOperatorRuntime` is the server-owned invocation boundary: it requires an
exact request-bound tenant/actor permission snapshot, requires `modules:manage`, rejects
cross-tenant run requests, and derives cancellation tenant only from the authorized
context.

Composition performs no SQL and starts no task. Transport adapters must not call the raw
shared replay runtime directly. Host scheduling, graceful task shutdown, authorized
GraphQL/HTTP/CLI commands, retry/backoff/dead-letter policy, in-page interruption,
locale/partition replay dimensions, Product and later source adapters, retained
crash/reclaim/restart evidence, reconciliation, and drift repair remain open.

## Status

- Rewrite: `in_progress`
- Current milestone: `M6 - replay runtime host composition and operator guard`
- FFA: `in_progress`
- FBA: `in_progress`
- M0 code reset: `complete`
- M1 generic core: `complete`
- M2 PostgreSQL storage benchmark: `complete`
- M2 accepted storage model: `JSONB`
- M2 rejected prototype cleanup: `complete`
- M3 storage-schema foundation: `complete`
- M3 atomic mutation persistence: `complete`
- M3 schema-application leases: `complete`
- M3 secondary-index lifecycle: `complete`
- M3 partition admission and evidence tooling: `complete`
- M3 retained packet execution: `open`
- M4 query engine and PostgreSQL adapter: `source_complete`
- M4 retained live PostgreSQL/reference equivalence: `open`
- M5/M6 bounded source replay contract: `source_complete`
- M6 one-page replay/checkpoint progression: `source_complete`
- M6 job leases and checkpoint attempt fencing: `source_complete_owner_execution_pending`
- M6 bounded multi-page replay and cancellation: `source_complete_owner_execution_pending`
- M6 replay runtime host composition and operator guard: `source_complete_owner_execution_pending`
- M6 scheduler, graceful shutdown, retry/DLQ, commands, and source adapters: `open`
- Production consumer and partition lifecycle cutover: `forbidden_pending_evidence`

## Verification

The repository owner runs checks and database evidence during this rewrite:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- `cargo test -p rustok-index source_registry --lib -- --nocapture`
- `cargo test -p rustok-index source_replay --lib -- --nocapture`
- `cargo test -p rustok-index source_replay_job --lib -- --nocapture`
- `cargo test -p rustok-index source_replay_runner --lib -- --nocapture`
- `cargo test -p rustok-index replay_runtime --lib -- --nocapture`
- `cargo test -p rustok-server index_replay_runtime_composition -- --nocapture`
- `node scripts/verify/verify-index-query-contract.mjs`
- `node scripts/verify/verify-index-source-replay-contract.mjs`
- `node scripts/verify/verify-index-replay-job-leases.mjs`
- `node scripts/verify/verify-index-replay-multipage-runner.mjs`
- `node scripts/verify/verify-index-replay-runtime-composition.mjs`
- `node scripts/verify/index-storage-tooling.mjs contract`
- `node scripts/verify/index-storage-tooling.mjs fixtures`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`

## Related Documentation

- [Crate README](../README.md)
- [Live implementation plan](./implementation-plan.md)
- [M5/M6 bounded source replay contract](./m5-m6-source-replay-contract.md)
- [M6 replay job lease and fencing boundary](./m6-replay-job-leases.md)
- [M6 bounded multi-page replay runner](./m6-bounded-multipage-runner.md)
- [M6 replay runtime host composition](./m6-replay-runtime-composition.md)
- [M4 source-owned schema registry](./m4-source-schema-registry.md)
- [M4 query runtime composition](./m4-query-runtime-composition.md)
- [M4 PostgreSQL query port](./m4-postgres-query-port.md)
- [M4 many-link aggregate ordering](./m4-many-link-aggregate-ordering.md)
- [M2 storage benchmark contract](./storage-benchmark.md)
- [M2 storage evidence comparison](./storage-comparison.md)
- [M2 storage operational review](./storage-operational-review.md)
- [M2 replacement evidence runbook](./storage-evidence-runbook.md)
- [M3 partition evidence runbook](./partition-evidence-runbook.md)
- [Index Engine rewrite ADR](../../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Accepted storage ADR](../../../DECISIONS/2026-07-24-index-storage-layout.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
