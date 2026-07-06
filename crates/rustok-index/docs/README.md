# Documentation `rustok-index`

`rustok-index` is the core module of the platform for the centralized index/read-model
layer. Its purpose is not product-facing search UX, but denormalized storage,
ingestion and cross-module query substrate.

## Purpose

- publish the canonical index/read-model contract for the platform;
- keep ingestion, rebuild and consistency semantics inside the module;
- provide the host and other modules with a stable internal query substrate for cross-module reads.

## Responsibilities

- index storage and denormalized projection records;
- ingestion lifecycle: bootstrap, incremental sync, rebuild and drift control;
- link-aware filtering and cross-module query substrate;
- operator-facing health/rebuild controls for index state;
- absence of product-facing search ranking and full-text UX semantics.

## Integration

- depends on `rustok-core` and stable integration contracts from source modules;
- can be used by `apps/server` and other platform consumers as an internal query/read-model layer;
- FBA owner ports (`IndexReadModelPort`, `IndexRebuildPort`) use shared `rustok_api::PortContext`/`PortError` and `PortCallPolicy` instead of package-local deadline/error shims;
- adapter-side FBA guardrails include validation helpers for read/list/rebuild requests, tenant-scope guard `ensure_index_document_tenant_scope` and typed degraded-mode error `index.rebuild_disabled`;
- must not collapse with `rustok-search`: `search` may read projections, but `index` does not become a search module;
- event-driven consumers of the module are published through `IndexModule::register_event_listeners(...)` and assembled by the server from `ModuleRegistry`, not through a separate host-owned dispatcher path;
- current module-owned consumers include `content_indexer`, `product_indexer` and `flex_indexer` for the standalone Flex read-model slice `index_flex_entries`;
- remains a `Core` module without its own storefront UX as a primary surface; operator-facing admin overview lives in `rustok-index-admin` and is structured as FFA `core` + native-only `transport` + `ui/leptos` adapter.

## Verification

- `cargo xtask module validate index`
- `cargo xtask module test index`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`
- targeted tests for ingestion, rebuild, link-aware queries and consistency semantics when changing the contract

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
