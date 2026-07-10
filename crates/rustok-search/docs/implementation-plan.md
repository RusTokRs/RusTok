# Implementation plan for `rustok-search`

Status: dedicated search module already works on PostgreSQL baseline; local
documentation and runtime boundary are aligned to a unified format.

## Execution checkpoint

- Current phase: catalog projection and Loco-free native admin runtime
- Last checkpoint: Next storefront host now uses host-owned `src/features/search` composition. Host passes route locale, tenant slug and enabled modules to registry render context, checks `product` is enabled, calls product-owned `apps/next-frontend/packages/rustok-product::fetchCatalogSearchOptions` over public GraphQL `storefrontCatalogSearchOptions(locale: String!)` and maps safe options into `SearchStorefrontPage`; search package does not import product internals. `rustok-search-admin` native server functions now use `HostRuntimeContext` for the database and typed `TransactionalEventBus` lookup, removing its Loco and outbox Loco-adapter package dependencies while preserving the GraphQL selected path.
- Next step: the next blocker before raising FBA above `boundary_ready` remains live runtime contract execution with real provider invocation.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block and the central readiness board.
- Last updated at (UTC): 2026-07-10T00:00:00Z


## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - FBA slice #3 switched `SearchQueryPort` and `SearchSuggestionPort` from ad-hoc deadline check to shared `PortCallPolicy::read()`, preserving locale propagation and typed `PortError` mapping without changing native/GraphQL transport.
  - module plan synchronized with central FFA/FBA readiness board;
  - FBA provider registry `crates/rustok-search/contracts/search-fba-registry.json` declares `SearchQueryPort`/`SearchSuggestionPort` (`search.query.v1`) for storefront/admin consumers with typed `PortContext`/`PortError`, read deadline semantics, degraded modes and fallback profiles;
  - static evidence `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`, executable no-compile runtime fallback smoke `crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json` + `scripts/verify/verify-search-fba-runtime-smoke.mjs`, runtime contract smoke `crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json` + `scripts/verify/verify-search-fba-runtime-contract.mjs`, invocation trace `crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json` + `scripts/verify/verify-search-fba-runtime-invocation.mjs`, fixture regression suites `scripts/verify/verify-search-fba-runtime-smoke.test.mjs` / `scripts/verify/verify-search-fba-runtime-contract.test.mjs` / `scripts/verify/verify-search-fba-runtime-invocation.test.mjs` and fast verifier `scripts/verify/verify-search-fba.mjs` maintain metadata/port/source drift without compilation; FBA status raised to `boundary_ready`, next status requires actual runtime contract execution;
  - further status promotion is done only together with verification evidence and local+central docs update;
  - Phase B slices #17-18 extracted admin route-query update semantics and preview form/request normalization into `admin/src/core.rs`; native/GraphQL transport was not modified;
  - Phase B slice #19 promoted reusable UI text/CSV and route-query update semantics to `rustok-api`, consumed by `leptos-ui-routing` and search admin core;
  - Phase B slice #20 moved render-ready labels, summary/preset text and result item representation of admin preview to `admin/src/core.rs`, leaving `admin/src/lib.rs` as Leptos render adapter without transport changes;
  - Phase B slice #21 moved analytics summary card value formatting to `SearchAnalyticsSummaryViewModel`, so the Leptos analytics panel no longer formats metrics inline;
  - Phase B slice #22 moved analytics query/intelligence table row formatting to core row view-models, preserving transport/native+GraphQL paths unchanged;
  - Phase B slice #23 moved diagnostics card state badge and newest-indexed summary to core view-model, leaving Leptos layer only for i18n labels and render;
  - Phase B slice #24 moved lagging diagnostics table row formatting to core row view-model, preserving transport/native+GraphQL paths unchanged;
  - Phase B slice #25 moved consistency diagnostics issue labels, badge classes, source/status labels and indexed fallback to core row view-model;
  - Phase B slice #26 added module-owned `admin/src/transport/` facade for bootstrap, preview, analytics, diagnostics, settings and dictionary operations; Leptos admin no longer calls `api::*` directly, and the native/GraphQL selected-path adapter now lives in `admin/src/transport/native_server_adapter.rs`;
  - Phase B slice #27 extracted `admin/src/ui/leptos.rs` and `storefront/src/ui/leptos.rs` as explicit render adapters; `admin/src/lib.rs` and `storefront/src/lib.rs` now only declare layers and re-export public entry points, without changing the native/GraphQL transport contract;
  - Phase B slice #28 added `SearchSynonymRowViewModel`, `SearchStopWordRowViewModel` and `SearchQueryRuleRowViewModel` to `admin/src/core.rs`; Leptos dictionaries tables no longer format synonyms summary, pinned position and document/source path inline;
  - Phase B slice #29 added `SearchSynonymMutationRequest`, `SearchStopWordMutationRequest` and `SearchPinRuleMutationRequest` builders to `admin/src/core.rs`; Leptos dictionaries forms now pass core-owned request objects to transport instead of inline parsing/request construction;
  - Phase B slice #30 added `SearchResultsLabels`, `SearchResultItemViewModel` and `SearchResultsViewModel` to `storefront/src/core.rs`; Leptos storefront results no longer format summary/preset/locale/source/score/snippet inline.
  - Phase B slice #31 split storefront transport facade into `native_server_adapter` and `graphql_adapter`; build-profile-selected orchestration now lives in `storefront/src/transport/mod.rs`, native endpoints live in `storefront/src/transport/native_server_adapter.rs`, GraphQL selected-path execution lives in `storefront/src/transport/graphql_adapter.rs`, and legacy `storefront/src/api.rs` is removed.
  - Phase B slice #32 added `SearchSuggestionsLabels`, `SearchSuggestionNavigation` and `SearchSuggestionItemViewModel` to `storefront/src/core.rs`; Leptos suggestions adapter no longer decides inline document-vs-query navigation or action/kind labels.
  - Phase B slice #33 added `SearchPresetChipViewModel`, preset chip class constants and builder to `storefront/src/core.rs`; Leptos preset chips no longer store selected/idle class strings and next-selection policy inline.
  - Phase B slice #34 added `SearchFacetGroupViewModel`, `SearchFacetBucketViewModel` and `build_search_facet_view_models` to `storefront/src/core.rs`; Leptos facet cards no longer format facet names/bucket labels inline.
  - Phase B slice #35 added `SearchResultActionViewModel` and `build_search_result_action_view_model` to `storefront/src/core.rs`; Leptos result cards no longer decide no-target/open-link labels, href state or click-tracking position inline.
  - Phase B slice #36 added `SearchEmptyStateViewModel`, `SearchFeatureCardViewModel`, `build_search_empty_state_view_model` and `build_search_results_feature_cards` to `storefront/src/core.rs`; Leptos empty/feature cards no longer own title/body presentation objects.
  - Phase B slice #37 added `SearchResultsHeaderViewModel` to `storefront/src/core.rs`; Leptos results header no longer assembles query label, query string, summary, preset and locale presentation inline.
  - Review of excessive extraction: proposed feedback-envelope and admin transport DTO pass-through slices rejected; the accepted boundary is that one-shot i18n success/error texts, adapter-local reset/refresh effects and mechanical unpacking of transport parameters remain in adapter/facade if there is no reusable policy semantics.
  - Phase B slice #38 added `StorefrontSearchFetchRequest`, `search_preview_filters_from_route` and `build_storefront_search_fetch_request` to `storefront/src/core.rs`; storefront Leptos resource no longer decides inline blank-query skip, query trim, preset normalization or route-filter-to-transport mapping, while native/GraphQL transport facade signatures did not change.
  - Phase B slice #39 added `StorefrontSearchRouteIntent` and `build_storefront_search_route_intent` to `storefront/src/core.rs`; storefront Leptos navigation no longer decides inline set/delete policy for `q` and `preset`, but only applies the prepared route intent to browser URL.
  - Phase B slice #40 added `DEFAULT_SUGGESTION_MIN_LEN`, `StorefrontSuggestionFetchRequest` and `build_storefront_suggestion_fetch_request` to `storefront/src/core.rs`; storefront Leptos suggestions resource no longer owns inline autocomplete min-length gate or query trim policy.
  - Phase B closure decision: search FFA will not be expanded further without a new functional surface; the current code split is sufficient for `phase_b_ready`, and further work is moved to parity/evidence hardening.
  - Slice #41 evidence hardening added `verify-search-ui-boundary.mjs` and fixture tests checking admin/storefront crate-root wiring, Leptos-free core helpers, prohibition of raw `api::*`/adapter calls from UI, storefront build-profile-selected native/GraphQL split and absence of legacy storefront `api.rs`; latest update also forbids legacy admin `api.rs` and pins `admin/src/transport/native_server_adapter.rs`.
  - Catalog projection search slice connected `PgSearchEngine` to product-owned highload projections without dependency on Rust types from `rustok-index`: category filters use `index_product_categories`, virtual categories participate as materialized assignments, attribute filters/sorts/facets use `index_product_attribute_values` with explicit `channel_id` scope, detached rows are excluded, and facet buckets preserve stable key and optional localized label.
  - Catalog projection guardrail slice extended `scripts/verify/verify-search-ui-boundary.mjs`: source-lock checks `SearchQuery`/GraphQL optional catalog fields, `SearchAttributeFilter`, facet `label`, reading `index_product_categories`/`index_product_attribute_values`, explicit channel scope, dynamic `attr:<code>` facets and pinned-rule skip with catalog filters; `npm run test:verify:search:ui-boundary` covers missing catalog markers.
  - UI transport parity slice extended Leptos admin/storefront DTO with all optional catalog filters/sort fields, routed them through admin native `#[server]` and parallel GraphQL adapters, added route mapping for storefront category/channel/sort parameters and localized facet label with fallback to stable value. `verify-search-ui-boundary.mjs` source-locks the new contract without package-local locale fallback.
  - Native admin runtime cutover: `admin/src/transport/native_server_adapter.rs` resolves its database through `HostRuntimeContext` and resolves `TransactionalEventBus` only for the two write flows that publish events. The package no longer depends on `loco-rs` or `rustok-outbox/loco-adapter`; GraphQL remains the parallel selected transport.
- Last verified at (UTC): 2026-07-02T00:00:00Z
- Owner: `rustok-search` module team

## Scope of work

- maintain `rustok-search` as a separate core module for search UX and engine semantics;
- do not mix search responsibilities with `rustok-index`;
- synchronize backend contract, admin/storefront surfaces and observability.

## Current state

- the module already owns `search_documents`, analytics storage, dictionaries and query rules;
- PostgreSQL FTS and `pg_trgm` serve as the baseline engine contract;
- live PostgreSQL query-plan gate confirms GIN paths on 100,000 rows:
  FTS `6.627 ms`, typo fallback `327.516 ms`; typo candidates are collected through
  `UNION` of indexed fields without parallel sequential scan;
- Leptos and Next admin surfaces are already connected, storefront path exists on the same backend contract;
- rebuild, diagnostics, analytics and settings editor already constitute a working operator baseline.
- operator-plane contract is further maintained through `xtask`: public exports, README markers and `docs/observability-runbook.md` must not degrade during further refactoring.
- boundary `index != search` is further maintained through a contract check in `xtask` so the search surface does not revert to index-owned runtime types.

## Stages

### 1. Contract stability

- [x] lock boundary `index != search`;
- [x] maintain PostgreSQL as baseline engine and settings-driven engine selection;
- [x] keep admin/storefront surfaces on a unified backend contract;
- [x] Expand capability matrix and contract tests;
- [x] Finalize search-facing error catalog and validation policy;
- [ ] maintain sync between runtime metadata, UI packages and diagnostics surfaces.

### 2. Product hardening

- [ ] finalize richer sorting/profile controls and advanced storefront UX polish;
- [x] Connect PostgreSQL search to channel-scoped normalized product facets/sorts and materialized virtual category assignments.
- [x] Lock projection-search contract with fast source/schema guardrail.
- [x] Connect UI controls and route/query contract to optional catalog filters/sort fields.
- [ ] develop retry/DLQ strategy for ingestion/rebuild pipeline;
- [ ] complete admin dashboards and production-grade analytics presentation.

### 3. Connector expansion

- [ ] add external connector crates for Meilisearch, Typesense and Algolia;
- [ ] lock degraded-mode and fallback contract for optional engines;
- [ ] document health/schema-sync guarantees for connector path.

## Verification

- `cargo xtask module validate search`
- `cargo xtask module test search`
- targeted tests for ingestion, ranking, diagnostics, dictionaries and storefront/admin query flows

## Update rules

1. When changing search runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing search/index boundary, synchronize ADR and related docs.
4. When changing metadata, UI packages or engine selection contract, synchronize `rustok-module.toml`.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [x] Lock/update verification gates for current module state (`npm run verify:search:ui-boundary`, `npm run test:verify:search:ui-boundary`).


## FFA/FBA migration status

The completed core/transport/ui extraction, catalog-filter host composition, and embedded-provider FBA baseline are documented in the module README and evidence contracts. `verify-search-ui-boundary.mjs` and `npm run verify:search:fba` protect the current boundary; live provider execution remains the FBA promotion blocker.
