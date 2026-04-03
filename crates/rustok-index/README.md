# rustok-index

## Purpose

`rustok-index` owns read-model and indexing contracts for RusToK.

## Responsibilities

- Provide `IndexModule` metadata for the runtime registry.
- Define indexer traits and indexing runtime contracts.
- Own index migrations and index rebuild helpers.
- Serve as the long-term cross-module index/read-model substrate rather than the
  product-facing search module.

## Interactions

- Depends on `rustok-core` for module contracts.
- Consumes domain events published by content, commerce, blog, forum, pages, and workflow paths.
- Content-node tag extraction now reads `nodes.metadata.tags` directly and no longer depends on
  legacy `tags` / `taggables` joins from `rustok-content`.
- Used by `apps/server` runtime wiring for index rebuild and cross-module index integrations.
- Exposes a module-owned Leptos admin overview through `rustok-index-admin`.
- Does not publish its own RBAC surface.
- Admin access to indexing operations is enforced by `apps/server` through the permissions
  of the domain being managed, not through direct role checks inside the module.
- Current code still contains early search-oriented scaffolding that is planned to move
  into a dedicated search capability/module over time.

## Entry points

- `IndexModule`
- `Indexer`
- `LocaleIndexer`
- `IndexerContext`
- `IndexerRuntimeConfig`
