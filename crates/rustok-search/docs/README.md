# `rustok-search` Documentation

`rustok-search` — dedicated core search module of the platform. Local documentation
of the module must describe the search runtime itself, not mix it with `rustok-index`
or host-specific UI wiring.

## Purpose

- publish the canonical search API and runtime contracts;
- keep search document materialization, ranking and query normalization inside the module;
- evolve admin/storefront search surfaces over a common backend contract.

## Scope

- `search_documents` and related search-owned dictionaries/analytics storage;
- search query parsing, ranking, filter presets, typo tolerance and merchandising rules;
- admin/storefront query surfaces and module-owned UI packages;
- observability, rebuild and diagnostics for search state;
- optional connector model for external search engines.

## Integration

- remains a separate module from `rustok-index`: `search` is responsible for UX, ranking and engine semantics, not for the shared indexed read-model substrate;
- uses PostgreSQL as the baseline engine and may be extended by separate connector crates;
- publishes module-owned migrations `search_settings`, `search_documents`, query analytics, dictionaries and typo-tolerance indexes; the server migrator must include them as part of backend schema wiring, otherwise admin/storefront search bootstrap is not considered operational;
- must keep Leptos and Next UI surfaces on the same backend contract;
- GraphQL query/mutation/types live in `rustok-search`; `apps/server` only composes roots and passes host runtime context, including the rate-limit adapter;
- event-driven ingestion is published by the module via `SearchModule::register_event_listeners(...)` and connected by the server through `ModuleRegistry`, without a separate host-owned search dispatcher;
- domain modules deliver changes through the ingestion path without knowing about the active engine.

## Projection correctness

- Search projector operations are tenant-scoped: ingestion always takes `tenant_id` from `EventEnvelope`, and `PgSearchEngine` requires `SearchQuery.tenant_id`.
- Re-delivery of events must not corrupt the read model: the projector performs a scoped delete + rebuild/upsert in a transaction, and materialized rows are written via stable `document_key`.
- `search_documents.document_key` is the primary key; content/product materialization uses `ON CONFLICT (document_key) DO UPDATE`, so a repeated upsert updates the existing row rather than creating a duplicate.
- Product catalog search reads normalized high-load projections built by `rustok-index`: `index_product_categories` for primary/additional/materialized virtual category assignments and `index_product_attribute_values` for effective attribute facet/search/sort rows.
- GraphQL search input supports optional `channelId`, `categoryIds`, `attributeFilters`, `sortAttributeCode` and `sortDesc`. If `channelId` is not set, the PostgreSQL engine reads only global rows (`channel_id IS NULL`); if set, it reads only rows for that channel without a fallback chain.
- Leptos admin/storefront DTO support the same catalog filters/sort fields: admin native `#[server]` is the primary internal transport, GraphQL remains parallel. Localized facet labels come from the projection for the effective locale set by the host, without package-local query/header/cookie fallback.
- Leptos and Next admin/storefront surfaces show catalog filter/sort controls over the same contract. The Leptos storefront stores selection state in the URL via typed `snake_case` query keys (`channel_id`, `category_ids`, `attribute_code`, `attribute_values`, `sort_attribute_code`, `sort_desc`), while Next packages receive locale only from host/runtime props.
- Search UI surfaces support picker-ready host metadata: the host can pass category/attribute option lists, and Leptos/Next packages show datalist hints without a direct import from `rustok-product`. The real source of these options must remain product-owned public/admin transport.
- The Next admin host already passes real category/attribute options to the search playground via product-owned GraphQL helpers from `packages/rustok-product`; labels are requested with the host effective locale, and on error the metadata search surface remains available without hints.
- Category filtering works through `index_product_categories`, so materialized virtual categories participate in listing/search just like structural/collection assignments.
- Attribute facets are built only from effective `is_filterable = TRUE` and `is_detached = FALSE` rows. The bucket `value` remains a stable `facet_bucket_key`, and optional `label` returns a localized caption from the projection for clients that need display text.
- Exact-query pinned rules do not load additional raw `search_documents` items with catalog filters; a pinned result can only rise if it is already in the filtered result set.
- Restart recovery is performed via `SearchProjector::ensure_bootstrap`: if there are no `search_documents` for the tenant, a tenant-wide rebuild is triggered.
- Migration `m20260324_000002_create_search_documents` creates `search_vector`, the `tsvector` update trigger, GIN index `idx_search_documents_fts` and tenant-aware btree indexes `idx_search_documents_lookup` / `idx_search_documents_entity`.
- Migration `m20260325_000006_add_search_typo_tolerance_indexes` includes `pg_trgm` and creates GIN trigram indexes for `title`, `slug`, `handle` and `keywords_text`.
- GiST index is not used for the current PostgreSQL baseline: FTS and typo-tolerant paths are designed for GIN indexes. If a GiST-specific search strategy appears, it must be a separate migration with query-plan evidence.
- Live PostgreSQL gate `tests/postgres_query_plan.rs` creates 100 000 temporary documents, runs `EXPLAIN (ANALYZE, BUFFERS)` and checks GIN FTS/trigram indexes. Baseline from 2026-06-27: FTS `6.627 ms`, typo fallback `327.516 ms`.
- Typo fallback builds candidates via `UNION` of four indexed branches (`title`, `slug`, `handle`, `keywords_text`) so that the overall `OR` does not degrade into a parallel sequential scan.
- Product-owned Leptos admin transport publishes neutral `fetch_catalog_search_options`: the current-tenant native `#[server]` endpoint is the primary path, GraphQL remains a parallel fallback. `apps/admin::SearchAdminComposition` already passes host effective locale/auth/tenant, checks that `product` is enabled, and maps options to a public search DTO without importing product internals inside the search UI.
- Product-owned Leptos storefront transport separately publishes public-safe category/attribute options via native `#[server]` first and GraphQL `storefrontCatalogSearchOptions(locale: String!)`. `apps/storefront::SearchStorefrontComposition` passes the host locale, checks that `product` is enabled, and maps owner DTOs to `SearchCatalogFilterOption`; the search storefront package does not import product internals.
- The Next storefront repeats the same boundary via `apps/next-frontend/src/features/search`: the host passes route locale/tenant slug, product-owned `apps/next-frontend/packages/rustok-product` reads `storefrontCatalogSearchOptions(locale: String!)`, and the search package receives only category/attribute option props.

## Verification

- `cargo xtask module validate search`
- `cargo xtask module test search`
- `cargo test -p rustok-search -- --include-ignored --nocapture` with live PostgreSQL `DATABASE_URL`
- targeted tests for query normalization, ranking profiles, rebuild flows and diagnostics surfaces

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Observability runbook](./observability-runbook.md)
- [ADR: boundary `index != search`](../../../DECISIONS/2026-03-29-index-search-boundary.md)
