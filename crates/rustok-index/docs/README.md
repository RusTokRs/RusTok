# Documentation `rustok-index`

`rustok-index` is the platform-owned cross-module relational Index Engine. It
addresses the same problem class as the Medusa Index Module: source modules
publish generic schemas, records, mutations, and links; Index materializes them
into optimized relational storage and serves structured cross-module queries
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
- SQL planning/compilation;
- rebuild, checkpointing, reconciliation, and drift repair;
- operator health, lag, failure, and rebuild controls.

## Excluded scope

- text relevance and ranking;
- typo tolerance, synonyms, autocomplete, and search UX;
- external search-engine connectors;
- source-module table reads from Index core;
- source-specific Product, Content, or Flex logic in the engine.

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
BUFFERS, full-plan, and node-level WAL evidence.

The persistent maintenance harness provides committed update plus
delete/reinsert cycles, exact cardinality guards, baseline/after-churn/after-
VACUUM schema-size and `pg_stat_user_tables` snapshots, and ordinary
`VACUUM (ANALYZE)` duration. It intentionally does not rely on `VACUUM FULL`.

The archived decision benchmark preserved full module/entity/schema-version
identity across all measured candidates. After the ADR selected JSONB, the
rejected typed-EAV and hot-projection implementations were deleted. The remaining
JSONB path is a selected-layout regression harness, not production persistence.

The operational review evaluates genericity, schema evolution, index and
migration management, mutation/query complexity, diagnostics, rebuild, and
partitioning independently from benchmark timings. It treats the hot typed projection as a best-case baseline rather than an
eligible canonical generic model. Replacement same-commit evidence selected JSONB
over typed EAV because JSONB preserved the generic boundary with materially lower
size, load, mutation, keyset/count, and maintenance cost.

The archived smoke and original 100k packets remain historical diagnostics.
Actions run `30222913450` produced validated replacement `100k`/`1m` packets and
a decision-ready comparison on one commit. The accepted storage ADR selects JSONB.

## M3 storage schema foundation

The module-owned migration source now creates seven generic tables without
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

The first runtime persistence slice is now implemented by `PostgresMutationStore`.
A `MutationDelivery` is validated against the in-memory `SchemaRegistry`, claimed
through the tenant/source/delivery inbox key, serialized by a transaction-scoped
PostgreSQL advisory lock on the complete entity key, compared with the current
source version, and then applied as one transaction. Live upserts replace
the JSONB field payload plus ordered links; deletes replace them with a tombstone.
Exact redelivery is idempotent, stale delivery is terminally ignored, delivery-ID
payload reuse fails closed, and any entity/link failure rolls back the inbox claim.

Schema application leases, partition/secondary-index lifecycle, PostgreSQL
Testcontainers/concurrency evidence, query execution, and batch ingestion remain
later M3/M4/M5 slices.

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
- Production persistence: mutation writes implemented; query adapter not yet implemented

## Verification

The repository owner runs the checks and database evidence during this rewrite:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`
- `cargo run -p rustok-benchmarks --bin index-storage-benchmark --release`
- `cargo run -p rustok-benchmarks --bin index-storage-mutation-benchmark --release`
- `cargo run -p rustok-benchmarks --bin index-storage-maintenance-benchmark --release`

## Related Documentation

- [Crate README](../README.md)
- [Live implementation plan](./implementation-plan.md)
- [M2 storage benchmark contract](./storage-benchmark.md)
- [M2 storage evidence comparison](./storage-comparison.md)
- [M2 storage operational review](./storage-operational-review.md)
- [Index Engine rewrite ADR](../../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
