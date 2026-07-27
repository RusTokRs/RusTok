# Documentation `rustok-index`

`rustok-index` is the platform-owned cross-module relational Index Engine. Source
modules publish generic schemas, records, mutations, and links; Index materializes
them into optimized relational storage and serves structured cross-module queries
without runtime fan-out.

## Purpose

- publish canonical schema, mutation, query, source, and rebuild contracts;
- keep ingestion, storage, planning, rebuild, and consistency semantics inside
  the module;
- provide server, storefront, admin, and `rustok-search` with a stable substrate
  for cross-module filtering, projection, sorting, count, and pagination;
- scale reads and rebuilds independently from source-module query paths.

## Responsibility Zone

- versioned schema and link registry;
- generic records and mutations;
- explicit tenant/locale query scope;
- registry-backed record and query validation;
- deterministic link graph and field paths;
- versioned keyset cursors;
- incremental ingestion and inbox deduplication;
- PostgreSQL storage and distributed coordination;
- schema application and secondary-index lifecycle;
- measured partition admission and shadow planning;
- retained partition evidence preparation, capture assembly, and validation;
- SQL planning/compilation;
- rebuild, checkpointing, reconciliation, and drift repair;
- operator health, lag, failure, and rebuild controls.

## Excluded scope

- text relevance and ranking;
- typo tolerance, synonyms, autocomplete, and search UX;
- external search-engine connectors;
- source-module table reads from Index core;
- source-specific Product, Content, or Flex logic in the engine;
- destructive partition cutover without retained PostgreSQL evidence.

## Integration

- source modules publish generic schemas, records, mutations, and rebuild streams;
- `IndexModule` contributes the canonical production migrations through the platform
  migration composition;
- server, storefront, admin, and `rustok-search` consume stable Index ports rather
  than reading Index tables directly;
- benchmark DDL and evidence remain isolated under `ops/benches` and never become
  runtime migrations.

## Rewrite policy

Backward compatibility with the rejected implementation is not a goal.
Conflicting code is deleted instead of preserved through compatibility layers.
M0 removed the complete source-specific implementation and its migrations,
contracts, runtime scheduler, server wiring, and admin table reads.

## Implemented core

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
- rejection of ambiguous ordering through `many` links;
- checksummed postcard/Base64 keyset cursors bound to query scope and schema
  fingerprint;
- a test-only mutation/query reference engine and property invariants for later
  PostgreSQL equivalence testing.

## M2 storage benchmark

Benchmark code lives outside the production crate in
`ops/benches/src/index_storage`. Candidate DDL is not a production migration or
runtime storage contract.

The read/query harness provides deterministic scale datasets, three storage
candidates, shared read workloads, cardinality checks, result-digest parity,
load/size measurement, and full JSON `EXPLAIN (ANALYZE, BUFFERS, WAL)` evidence.
The transactional mutation harness provides identical update/delete workloads,
affected entity/link parity, rollback isolation, and planning/execution,
BUFFERS, full-plan, and node-level WAL evidence. The persistent maintenance
harness provides committed update plus delete/reinsert cycles, exact cardinality
guards, baseline/after-churn/after-VACUUM schema-size and table-stat snapshots,
and ordinary `VACUUM (ANALYZE)` duration.

Replacement same-commit evidence selected JSONB over typed EAV and hot projection.
Rejected candidate implementations were deleted. The remaining JSONB path is a
selected-layout regression harness, not production persistence. Partitioning was
not part of M2 evidence, so the canonical relations remain unpartitioned by
default.

## M3 PostgreSQL storage engine

The module-owned migration source creates seven generic tables without
source-domain columns or benchmark schemas:

- `index_schemas` stores exact versioned schema JSON and fingerprints;
- `index_entities` stores the JSONB payload/tombstone envelope with complete
  tenant, module, entity, schema-version, entity-ID, and locale identity;
- `index_links` stores ordered independent links bound to the source entity's
  exact full-range `DECIMAL(20,0)` source version;
- `index_inbox` stores deduplication, mutation identity, processing leases, and
  terminal outcomes;
- `index_checkpoints` stores ingestion and rebuild cursors;
- `index_jobs` stores bounded durable schema/index/rebuild/reconciliation work;
- `index_consistency_findings` stores open/resolved drift findings.

`PostgresMutationStore` validates each `MutationDelivery`, claims the composite
inbox identity, serializes the complete entity key with a transaction-scoped
advisory lock, applies monotonic entity/tombstone and ordered-link replacement,
and completes the inbox in one transaction. Exact redelivery is idempotent,
stale delivery is terminally ignored, payload reuse fails closed, and write
failure rolls back the inbox claim.

`PostgresSchemaLeaseStore` coordinates exact tenant/schema application through
`schema_apply` jobs. It verifies persisted active schema/fingerprint state,
returns `Busy` or terminal `AlreadyApplied`, reclaims expired work with incremented
attempt fencing, and requires the exact current worker/attempt for heartbeat and
completion.

`SecondaryIndexPlan` derives deterministic indexes from the exact schema contract.
Scalar filterable/sortable fields use typed partial B-tree expressions. Filterable
`many` fields use field-local JSONB containment GIN. Expressions follow the
production tagged `IndexValue` payload through each field's `value` member. Stable
names bind tenant, schema identity/version/fingerprint, field type/cardinality,
index kind, and payload contract.

`PostgresSecondaryIndexManager` coordinates ensure, reindex, and retirement with
`secondary_index` jobs, transaction advisory locking, expiry reclaim, heartbeat,
and attempt fencing. PostgreSQL execution uses `CREATE INDEX CONCURRENTLY`,
`REINDEX INDEX CONCURRENTLY`, and `DROP INDEX CONCURRENTLY`. Owner comments bind
each index to its full definition hash, and completion checks `indisready` plus
`indisvalid`. Retirement remains available after schema retirement. SQLite is
contract-test-only.

`evaluate_partition_admission` compares one unpartitioned baseline with one exact
SHA-256 identified tenant-hash shadow packet under an explicit policy. It checks
measured rows, bytes, tenants, tenant-predicate coverage, entity/link digests,
catch-up, foreign keys, orphan links, query-plan regressions, p95 query/mutation
regressions, WAL amplification, partition-size skew, and cutover-lock duration.
Any failed gate returns `KeepUnpartitioned` with typed reasons.

An admitted `PartitionShadowPlan` derives stable names and bootstrap SQL only for
shadow hash-partition parents and children. Hash modulus must be a power of two
from 2 through 128. The plan cannot rename, drop, or alter production entity/link
relations. Copy, constraint/index attachment, replay or dual-write, durable global
operation ownership, cutover, rollback, and retained PostgreSQL evidence remain
open M3 work.

Partition evidence tooling binds one immutable manifest to a SHA-256 `evidence_id`,
emits deterministic shadow-only bootstrap SQL, and validates exact
query/mutation/maintenance/cutover repetition groups. The capture/assembly layer
reads six retained raw JSON artifacts once, confines unique relative paths to one
bundle, rejects symbolic links and aliases, calculates exact-byte SHA-256 hashes,
and publishes a structurally validated packet without accepting precomputed packet
fields or pass flags. Final validation calculates tenant coverage, digest parity,
latency regression, WAL amplification, partition skew, lock duration, rollback
state, and typed rejection reasons. The repository owner still executes and
retains the PostgreSQL packet.

PostgreSQL Testcontainers/concurrency evidence, query execution, and batch
ingestion remain later M3/M4/M5 slices.

## Status

- Rewrite: `in_progress`
- Current milestone: `M3 - PostgreSQL storage engine`
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
- M3 partition admission and shadow planning: `complete`
- M3 partition evidence packet tooling: `complete`
- M3 partition evidence capture/assembly: `complete`
- Production persistence: mutation writes, schema/index coordination, partition
  admission, evidence assembly, and evidence validation implemented; query adapter,
  retained PostgreSQL partition run, and partition cutover lifecycle not yet
  implemented

## Verification

The repository owner runs the checks and database evidence during this rewrite:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- `node scripts/verify/index-storage-tooling.mjs contract`
- `node scripts/verify/index-storage-tooling.mjs fixtures`
- `node --test scripts/verify/index-partition-evidence-assembly.test.mjs`
- `node scripts/verify/verify-index-secondary-index-lifecycle.mjs`
- `node scripts/verify/verify-index-partition-admission.mjs`
- `node scripts/verify/verify-index-partition-evidence.mjs`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`

## Related Documentation

- [Crate README](../README.md)
- [Live implementation plan](./implementation-plan.md)
- [M2 storage benchmark contract](./storage-benchmark.md)
- [M2 storage evidence comparison](./storage-comparison.md)
- [M2 storage operational review](./storage-operational-review.md)
- [M3 partition evidence runbook](./partition-evidence-runbook.md)
- [Index Engine rewrite ADR](../../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Accepted storage ADR](../../../DECISIONS/2026-07-24-index-storage-layout.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
