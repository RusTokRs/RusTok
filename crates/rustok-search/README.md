# rustok-search

## Purpose

`rustok-search` owns product-facing search contracts for RusToK.

## Responsibilities

- Provide `SearchModule` metadata for the runtime registry.
- Define canonical search engine selection and connector metadata.
- Keep PostgreSQL as the default search engine contract.
- Own search settings persistence and future connector-facing runtime contracts.
- Own module-local `search_documents` storage and execute PostgreSQL FTS with `tsvector`,
  `websearch_to_tsquery`, `ts_rank_cd`, highlights, and facet aggregation.
- Serve as the home for storefront/admin/global-search capabilities, separate from `rustok-index`.

## Interactions

- Depends on `rustok-core` for module contracts.
- May ingest from domain tables or neutral read models, but keeps its own search storage and runtime.
- Is used by `apps/server` as a core platform capability.
- Publishes module-owned admin and storefront UI packages for host composition.
- Keeps external engine integrations behind dedicated connector crates, rather than forcing domain modules to talk to provider SDKs directly.

## Entry points

- `SearchModule`
- `SearchEngineKind`
- `SearchConnectorDescriptor`
- `SearchSettingsRecord`
- `PgSearchEngine`

## Current status

- Admin GraphQL exposes engine availability, effective settings preview, and an FTS-backed
  `searchPreview` surface with filters and facets.
- Admin GraphQL also exposes dedicated `adminGlobalSearch`, so host-level admin
  quick search does not get mixed into control-plane preview telemetry.
- Admin GraphQL and both admin packages now ship a real settings editor for
  `active_engine`, `fallback_engine`, and tenant-local JSON config persistence.
- Storefront GraphQL exposes public `storefrontSearch`, limited to published content/products.
- Search queries are read-only and no longer trigger bootstrap rebuilds on the request path.
- PostgreSQL search reads from `search_documents`, not from `rustok-index` tables.
- Search GraphQL now also exposes `searchDictionarySnapshot` plus admin mutations
  for synonyms, stop words, and exact-query pinned-result rules.
- Query rewrite now applies tenant-owned stop words and synonym expansion before
  PostgreSQL FTS execution, while exact-query pin rules can promote curated
  results on both admin preview and storefront search.
- `SearchIngestionHandler` updates `search_documents` asynchronously from domain events and supports rebuild requests.
- Search rebuilds now execute transactionally so operators do not see half-rebuilt tenant indexes.
- Admin GraphQL exposes `searchDiagnostics`, `searchLaggingDocuments`, and
  `triggerSearchRebuild` for lag/state inspection, raw stale-document diagnostics,
  and queued tenant-wide or scoped rebuilds.
- Admin GraphQL now also exposes `searchAnalytics` backed by module-local
  `search_query_logs`, including top queries and zero-result analysis.
- Search GraphQL now exposes `trackSearchClick`, and result payloads include
  `queryLogId` plus a best-effort target URL so CTR and abandonment can be
  measured from real result clicks.
- Search GraphQL now enforces tenant-local scope and validates engine/filter input before execution.
- Search ingestion now runs with dispatcher retries, and diagnostics treat truly empty tenants as healthy instead of degraded.
- Prometheus telemetry now exposes query volume, latency, zero-result, indexing,
  and fleet-level lag metrics for `rustok-search`.
- The module now ships a local observability runbook for rebuilds, lag triage,
  and `/metrics` interpretation.
- Leptos admin and storefront packages are wired to the live GraphQL search contract.
- The Leptos admin package now ships separate overview, playground, diagnostics,
  analytics, and dictionaries surfaces under the module route, including live
  settings, synonym/stop-word/pin-rule editors, CTR, abandonment, low-CTR,
  and query-intelligence views, and is also exposed as a first-class admin
  sidebar entry.
- The Leptos admin host now also uses `rustok-search` for header-level global
  admin search with module-aware quick navigation and a fallback into the full
  search control plane.
- The Next admin package mirrors the same control-plane surfaces and uses the same
  GraphQL contract for settings, diagnostics, analytics, click tracking,
  rebuilds, FTS preview, and search dictionary management.
- The Next admin host now wires `rustok-search` into KBar so global admin
  search and command-palette quick-open use the same search contract and
  analytics surface.
