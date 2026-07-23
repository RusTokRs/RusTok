# rustok-index

## Purpose

`rustok-index` is RusToK's cross-module relational Index Engine. Source modules
publish indexable schemas, records, mutations, and links; Index materializes
them into optimized storage and executes filtering, projection, sorting,
counting, and pagination without runtime fan-out to source modules.

The current source-specific CQRS/read-model implementation is being replaced.
Backward compatibility with rejected internal Index APIs is not a rewrite goal.

## Responsibilities

- Own the generic index schema and link registry.
- Own incremental ingestion, deduplication, rebuild, reconciliation, and drift
  control.
- Own PostgreSQL index storage and distributed coordination.
- Validate and plan cross-module queries.
- Compile filtering, projection, sorting, count, and pagination to Index storage
  queries.
- Publish stable query, rebuild, and operator contracts.
- Keep product-facing search relevance and ranking in `rustok-search`.

## Boundaries

- Index core must not depend on Product, Content, Flex, Pricing, Inventory, or
  other source-domain crates.
- Source modules own conversion from domain state/events into generic Index
  records and mutations.
- Index must not read source-module tables directly.
- `rustok-search` may consume Index contracts but owns ranking, typo tolerance,
  autocomplete, synonyms, search UX, and external search-engine connectors.

## Rewrite status

- Current milestone: `M0 - hard reset and architecture lock`
- FFA status: `in_progress`
- FBA status: `in_progress`
- Legacy `index.read_model.v1` / `index.rebuild.v1` contracts remain temporary
  until the new engine boundary is implemented and consumers are migrated.

## Entry points

During the rewrite, new domain types live under `rustok_index::domain`. Existing
`Indexer`, `LocaleIndexer`, `IndexReadModelPort`, and `IndexRebuildPort` APIs are
legacy and scheduled for removal.

## Docs

- [Module documentation](./docs/README.md)
- [Live implementation plan](./docs/implementation-plan.md)
- [Index Engine rewrite ADR](../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Platform docs index](../../docs/index.md)
