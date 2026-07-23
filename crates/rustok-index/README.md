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
- Source modules own conversion from domain state/events into generic Index
  records and mutations.
- Index must not read source-module tables directly.
- `rustok-search` owns ranking, typo tolerance, autocomplete, synonyms, search
  UX, and external search-engine connectors.

## Rewrite status

- Current milestone: `M0/M1 - hard reset and domain core`
- FFA status: `in_progress`
- FBA status: `in_progress`
- Legacy `index.read_model.v1` and `index.rebuild.v1`: deleted

The current public engine surface is the database-independent
`rustok_index::domain` API. Source-specific indexers and migrations remain only
until the next M0 deletion pass.

## Entry points

- `IndexModule`
- `rustok_index::domain::*`
- `IndexSchema`, `IndexRecord`, `IndexMutation`, `IndexQuery`, and `FilterExpr`

## Docs

- [Module documentation](./docs/README.md)
- [Live implementation plan](./docs/implementation-plan.md)
- [Index Engine rewrite ADR](../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Platform docs index](../../docs/index.md)
