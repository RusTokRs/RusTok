# rustok-search implementation plan

## Purpose

This document is the canonical implementation roadmap for `rustok-search`.

It replaces the older central platform draft that previously lived in
`docs/modules/search-module-plan.md`. Search planning now lives with the actual
module code so architecture, runtime behavior, and rollout phases stay aligned.

## Objective

Build `rustok-search` as a dedicated core search module that:

- keeps search separate from `rustok-index`
- uses PostgreSQL as the default engine
- owns storefront and admin search surfaces
- evolves Leptos and Next UI in parallel on the same API contract
- supports optional external search connectors

## Architecture boundary

### `rustok-index`

`rustok-index` is responsible for:

- centralized index and read-model infrastructure
- ingestion and denormalized read models
- cross-module joins and filtering substrate
- rebuild, sync, and consistency workflows for index data

`rustok-index` is not responsible for:

- full-text search UX
- search ranking and relevance
- synonyms and stop words
- autocomplete and suggestions
- search analytics
- engine-specific search connectors

### `rustok-search`

`rustok-search` is responsible for:

- canonical search API and runtime contracts
- search document materialization
- query parsing and normalization
- ranking and relevance
- admin and storefront search UI
- global admin search
- search analytics and observability
- connector integration for external engines

## Runtime model

Preferred interaction model:

1. Domain modules emit changes.
2. `rustok-search` builds and updates its own `search_documents`.
3. Search UI and API talk only to `rustok-search`.
4. `rustok-search` may read from domain data directly and may optionally use
   neutral read models where that improves ingestion, but it does not depend on
   `rustok-index` as its canonical search storage.

Practical conclusion:

- `index` and `search` can cooperate, but they must not collapse into one module.
- `search -> index` can be allowed where useful.
- `index -> search` should remain forbidden.

## Engine strategy

### Default engine

PostgreSQL is the default and baseline engine:

- full-text search
- weighted `tsvector`
- `ts_rank` / `ts_rank_cd`
- `websearch_to_tsquery`
- `ts_headline`
- GIN indexes
- `pg_trgm`
- `unaccent`

### Optional engines

External engines should be delivered as dedicated optional connector crates:

- `rustok-search-meilisearch`
- `rustok-search-typesense`
- `rustok-search-algolia`

Recommended connector model:

- one connector per engine
- official SDK for each provider
- thin shared contract inside `rustok-search`
- engine selection through `rustok-search` settings

Why not a Laravel Scout style foundation:

- provider capabilities differ too much
- a single abstraction becomes either too weak or leaks provider behavior
- crate-based connectors fit the RusToK modular architecture better

## Engine selection UX

Expected operator experience:

1. A connector crate is added to the platform.
2. The connector registers capabilities and settings schema.
3. Search settings expose only actually installed engines.
4. `postgres` remains the default and fallback option.
5. Domain modules stay unaware of which engine is currently active.

## Current implementation checkpoint

Already implemented:

- dedicated `rustok-search` core module
- module-local `search_documents` storage
- module-local `search_query_logs` storage for persistent analytics
- module-local `search_query_clicks` storage for CTR and abandonment analysis
- module-local `search_synonyms`, `search_stop_words`, and `search_query_rules`
  storage for search dictionaries and merchandising rules
- PostgreSQL FTS runtime over `search_documents`
- canonical search contracts and settings persistence
- backend validation for `ranking_profiles` and `filter_presets`
- tenant-scoped GraphQL access and stricter search input validation
- async ingestion from content and product events
- queued rebuild flow for `search`, `content`, `product`, and optional target IDs
- transactional rebuild execution and dispatcher retries for ingestion safety
- Prometheus metrics for query volume, latency, zero-result rate, indexing
  outcomes, fleet-level lag/bootstrap visibility, storefront rate-limit
  outcomes, and audit-event publication status
- admin diagnostics via `searchDiagnostics`
- admin analytics via `searchAnalytics`
- admin dictionary snapshot via `searchDictionarySnapshot`
- admin CRUD mutations for synonyms, stop words, and exact-query pin rules
- live admin settings editor for active/fallback engine selection and JSON config
- public click tracking via `trackSearchClick`
- raw stale-document diagnostics via `searchLaggingDocuments`
- missing/orphaned projection diagnostics via `searchDiagnostics` counters and
  raw `searchConsistencyIssues`
- admin FTS preview via `searchPreview`
- dedicated host-level admin quick search via `adminGlobalSearch`
- public storefront search via `storefrontSearch`
- PostgreSQL typo-tolerant fallback over `pg_trgm` for zero-result searches
- built-in ranking profiles with per-surface defaults and admin preview override
- structured admin editors for ranking defaults and filter presets on both hosts
- read-only search query path without bootstrap rebuild side effects
- dedicated storefront rate limiting and outbox-backed audit events for
  settings/rebuild actions
- healthier diagnostics state model for truly empty tenants
- query-log-backed CTR, abandonment, low-CTR, and intelligence analytics
- slow-query rate analytics and slow-query leaderboard
- tenant-owned query normalization with stop-word removal and synonym expansion
- exact-query pinned-result merchandising rules applied on admin/storefront search
- local observability runbook for rebuilds, lag handling, and metrics
- Leptos admin control plane with overview, playground, diagnostics, dictionaries
- Next admin control plane mirroring the same contract
- first-class sidebar entry in both Leptos admin and Next admin
- Leptos header-level global admin search wired to `adminGlobalSearch`
- Next KBar global admin search wired to `adminGlobalSearch`
- fallback hand-off from host-level quick search into the full search control plane

Not implemented yet:

- analytics dashboards
- external connector crates

## Delivery phases

### Phase S0 - Architecture fixation

- [ ] Record ADR: `index != search`
- [x] Keep `rustok-search` as a dedicated core module
- [x] Keep dependency direction explicit
- [x] Define crate layout for core/admin/storefront packages
- [x] Keep engine selection centered in `rustok-search`

### Phase S1 - Core contracts

- [x] Define `SearchQuery`
- [x] Define `SearchResult`
- [x] Define `SearchEngine`
- [x] Define settings-driven engine selection
- [ ] Expand capability matrix and contract tests
- [ ] Finalize search-facing error catalog and validation policy

### Phase S2 - PostgreSQL MVP

- [x] Implement PostgreSQL as default engine
- [x] Use PostgreSQL FTS primitives first
- [x] Add filters and facets for `entity_type`, `source_module`, `status`
- [x] Add highlights/snippets
- [x] Finish ranking profile hardening
- [ ] Add broader sorting/profile controls

### Phase S3 - Ingestion pipeline

- [x] Materialize `search_documents`
- [x] Add event-driven upsert/delete
- [x] Add queued rebuild flows
- [ ] Add retry / DLQ strategy
- [x] Add consistency checks between source state and search state
- [ ] Add richer rebuild scopes such as locale-wide reindexing where needed

### Phase S4 - API surfaces

- [x] Add GraphQL search queries
- [x] Add admin/debug queries
- [x] Add dedicated `adminGlobalSearch` query for host-level quick search
- [x] Add rebuild mutation
- [x] Add settings mutation/editor flow beyond preview
- [ ] Decide whether any REST diagnostics endpoints are still needed

### Phase S5 - Storefront UI

- [x] Ship Leptos storefront package
- [x] Ship Next storefront package scaffold
- [x] Keep storefront search on shared backend contract
- [x] Add suggestions/autocomplete
- [ ] Add richer URL/state model and SSR polish

### Phase S6 - Admin UI

- [x] Ship Leptos admin package
- [x] Ship Next admin package
- [x] Add overview, playground, analytics, diagnostics, dictionaries surfaces
- [x] Add scoped rebuild UI
- [x] Add raw lagging document diagnostics
- [x] Add raw consistency diagnostics for missing/orphaned projection records
- [x] Add engine/settings editor
- [x] Build global admin search experience

### Phase S7 - Standard search features

- [x] Stop words
- [x] Synonyms
- [x] Query normalization policy
- [x] Typo tolerance
- [x] Suggestions/autocomplete
- [x] Filter presets
- [x] Ranking profiles
- [ ] Boost/bury rules
- [x] Query rules (exact pinned-result rules)
- [ ] Redirects
- [x] Audit trail for search settings changes

### Phase S8 - Analytics and observability

- [x] Query volume metrics
- [x] Search latency metrics
- [x] Zero-result rate
- [x] Slow-query rate
- [x] Indexing lag metrics
- [x] Failed indexing rate
- [x] Storefront rate-limit outcome metrics
- [x] Audit-event publication metrics
- [x] Top queries and zero-result query analysis
- [x] CTR and abandonment analysis
- [x] Query-intelligence candidates for tuning and merchandising work
- [ ] Admin dashboards
- [x] Operator runbooks

### Phase S9 - External engines

- [ ] Add `rustok-search-meilisearch`
- [ ] Add `rustok-search-typesense`
- [ ] Add `rustok-search-algolia`
- [ ] Add connector health and schema sync
- [ ] Add degraded-mode and fallback behavior

### Phase S10 - Advanced features

- [x] Merchandising and pinned results
- [ ] Hidden results
- [ ] "Did you mean"
- [ ] Curated landing/query rules
- [ ] Per-surface ranking profiles
- [ ] A/B testing for relevance strategies
- [ ] Semantic or hybrid search as a late additive phase

## Search document baseline

Minimum search document shape should support:

- `id`
- `tenant_id`
- `locale`
- `entity_type`
- `source_module`
- `title`
- `subtitle`
- `body`
- `keywords`
- `facets`
- `visibility` / `status`
- `admin_url` / public URL
- `payload`
- `updated_at`

## Definition of done for first production-ready release

- [x] `rustok-search` exists as a dedicated core module
- [x] PostgreSQL works as the default engine
- [x] Search GraphQL API exists
- [x] Leptos and Next admin surfaces exist
- [x] Rebuild and diagnostics exist
- [x] Baseline filters/facets exist
- [ ] Storefront UX is fully polished on both hosts
- [x] Global admin search is implemented
- [x] Settings editor is complete
- [ ] Telemetry and analytics are fully production-ready

## Open questions

- Should any search ingestion paths still read from `rustok-index`, or should
  the module remain domain-event/domain-table first?
- Should synonyms and stop words be platform-level first, then tenant-level?
- When external engines are enabled, what degraded-mode UX should operators see?

## Documentation rules for this module

When architecture, API contracts, routing, tenancy behavior, observability, or
admin/storefront UI responsibilities change:

1. Update this file first.
2. Update `crates/rustok-search/README.md`.
3. Update `crates/rustok-search/docs/README.md` if the documentation map changes.
4. Update `docs/index.md` if central documentation links need to move.
