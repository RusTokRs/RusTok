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

- Depends on `rustok-core` for module contracts and `rustok-api` for shared transport-agnostic port context/error/policy primitives.
- Consumes domain events published by content, commerce, blog, forum, pages, workflow, and Flex standalone paths.
- Content-node tag extraction now reads `nodes.metadata.tags` directly and no longer depends on
  legacy `tags` / `taggables` joins from `rustok-content`.
- Used by `apps/server` runtime wiring for index rebuild and cross-module index integrations.
- Publishes its event-driven consumers through `IndexModule::register_event_listeners(...)` and the shared module event dispatcher path.
- Current live event-driven consumers are `content_indexer`, `product_indexer`, and `flex_indexer`.
- Exposes a module-owned Leptos admin overview through `rustok-index-admin`.
- Does not publish its own RBAC surface.
- Admin access to indexing operations is enforced by `apps/server` through the permissions
  of the domain being managed, not through direct role checks inside the module.
- Product-facing search belongs to `rustok-search`; this crate does not expose a second
  search-engine contract.

## Entry points

- `IndexModule`
- `Indexer`
- `LocaleIndexer`
- `IndexerContext`
- `IndexerRuntimeConfig`
- `IndexReadModelPort` / `IndexRebuildPort`
- `validate_index_read_request`, `validate_index_list_request`, `validate_index_rebuild_request` and `ensure_index_document_tenant_scope` for adapter-side FBA guardrails

## Docs

- [Module docs](./docs/README.md)
- [Runtime fallback smoke evidence](./contracts/evidence/index-runtime-fallback-smoke.json)
- [Platform docs index](../../docs/index.md)
