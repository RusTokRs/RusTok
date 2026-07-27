# rustok-index

## Purpose

`rustok-index` is RusToK's cross-module relational Index Engine. Source modules
publish generic schemas, records, mutations, and links; Index materializes them
into optimized storage and executes filtering, projection, sorting, counting,
and pagination without runtime fan-out to source modules.

Backward compatibility with the rejected source-specific implementation is not
a rewrite goal.

## Responsibilities

- Own the generic schema and link registry.
- Own incremental ingestion, deduplication, rebuild, reconciliation, and drift
  control.
- Own PostgreSQL index storage and distributed coordination.
- Validate and plan cross-module queries.
- Compile projection, filtering, sorting, count, and pagination to Index storage
  queries.
- Publish stable query, source, rebuild, and operator contracts.
- Keep product-facing relevance and ranking in `rustok-search`.

## Boundaries

- Index core must not depend on Product, Content, Flex, Pricing, Inventory, or
  other source-domain crates.
- Source modules own conversion from domain state/events into generic records and
  mutations.
- Index must not read source-module tables directly.
- `rustok-search` owns ranking, typo tolerance, autocomplete, synonyms, search
  UX, and external search-engine connectors.
- The selected JSONB regression DDL lives under `ops/benches`; it is not a
  production migration or runtime storage contract. Historical three-candidate
  evidence is archived under `docs/evidence/`.

## Rewrite status

- Current milestone: `M3 - PostgreSQL storage engine`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: complete
- M1 generic domain/application core: complete
- M2 PostgreSQL storage benchmark: complete
- M2 accepted storage model: JSONB
- M2 rejected prototype cleanup: complete

All legacy ports, adapters, source indexers, projections, migrations, runtime
configuration, scheduler, errors, and server composition have been deleted.
Production persistence is intentionally absent until M3 implements the accepted
JSONB storage envelope.

## Current entry points

- `IndexModule`
- `rustok_index::domain::*`
- `rustok_index::application::*`
- `SchemaRegistry`, `IndexSchema`, `IndexRecord`, and `IndexMutation`
- `IndexQuery`, `IndexQueryScope`, `FilterExpr`, and typed `FieldPath`
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
- no ambiguous ordering through `many` links;
- checksummed keyset cursors bound to tenant, schema, fingerprint, locale, and
  order shape;
- reference mutation/query engine and property-based invariants for future
  PostgreSQL equivalence tests.

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
- [M2 storage benchmark contract](./docs/storage-benchmark.md)
- [M2 replacement evidence runbook](./docs/storage-evidence-runbook.md)
- [Index Engine rewrite ADR](../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Platform docs index](../../docs/index.md)
