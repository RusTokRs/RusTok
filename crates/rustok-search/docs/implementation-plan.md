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


## FFA pilot migration tracker (rustok-search)

- [x] Slice 1 scope locked (single use-case): query/filter input normalization (`parse_csv`, `optional_text`).
- [x] Storefront surface updated.
- [x] Admin surface checked/updated for the same use-case.
- [x] GraphQL selected-path parity confirmed (no contract regression): transport path not modified in this slice.
- [x] Double documentation verification completed.

- [x] Slice 2: storefront/admin facet display normalization moved to core (`facet_display_name`).
- [x] Slice 3: storefront/admin facet bucket label formatting moved to core (`facet_bucket_label`).
- [x] Slice 4: storefront/admin snippet fallback rendering moved to core (`snippet_or_fallback`).
- [x] Slice 5: storefront/admin score label normalization moved to core (`score_label`).
- [x] Slice 6: storefront/admin entity-source/status label formatting moved to core (`entity_source_label`, `source_entity_status_label`).
- [x] Slice 7: admin preview score-template value extraction switched to dedicated core helper (`score_value`).
- [x] Slice 8: storefront/admin error message composition moved to core (`error_with_context`).
- [x] Slice 9: storefront/admin score rendering unified to direct core helpers (`score_label`) without template/trim hacks.
- [x] Slice 10: admin relevance editor JSON formatting and ranking/filter preset extraction moved to core (`pretty_json_string`, `parse_json_for_editor`, `extract_ranking_profile_value`, `extract_surface_presets_json`).
- [x] Slice 11: admin analytics/diagnostics metric formatting moved to core (`format_days`, `format_percent_fraction`, `format_milliseconds`, `format_decimal_1`, `format_seconds`, `document_source_path`).
- [x] Slice 12: admin preview summary/preset rendering and diagnostics fallback text moved to core (`render_preview_summary`, `render_preview_preset`, `value_or_fallback`, `label_value_summary`).
- [x] Slice 13: admin analytics/dictionaries error messages and timestamp fallbacks switched to existing core helpers (`error_with_context`, `value_or_fallback`).
- [x] Slice 14: admin tab and diagnostics/consistency badge CSS class mapping moved to core (`tab_class`, `diagnostics_state_badge_class`, `consistency_issue_badge_class`).
- [x] Slice 15: admin navigation href, engine option label and rebuild feedback rendering moved to core (`module_overview_href`, `module_section_href`, `engine_option_label`, `rebuild_target_suffix`, `render_rebuild_feedback`).
- [x] Slice 16: admin relevance editor merge and JSON-array validation moved to core (`RelevanceEditorConfigInput`, `RelevanceEditorMessages`, `merge_relevance_editor_config`, `parse_json_array_for_editor`).
- [x] Slice 17: admin preview route-query update semantics moved to core (`RouteQueryUpdate`, `route_query_update`) without native/GraphQL transport changes.
- [x] Slice 18: admin preview form/request normalization moved to core (`SearchPreviewFormInput`, `SearchPreviewRequest`, `build_search_preview_request`), replacing the weak empty-state helper slice.
- [x] Slice 19: reusable UI text/CSV normalization and route-query update intent promoted to `rustok-api` (`normalize_ui_text`, `parse_ui_csv`, `UiRouteQueryUpdate`) and applied by `leptos-ui-routing`.
- [x] Slice 20: render-ready view-model admin preview panel moved to core (`SearchPreviewLabels`, `SearchPreviewViewModel`, `build_search_preview_view_model`), so Leptos panel only renders prepared fields and click actions.
- [x] Slice 21: render-ready values analytics summary cards moved to core (`SearchAnalyticsSummaryViewModel`, `build_search_analytics_summary_view_model`), so Leptos analytics panel passes already prepared strings to cards.
- [x] Slice 22: render-ready values analytics query/intelligence table rows moved to core (`SearchAnalyticsQueryRowViewModel`, `SearchAnalyticsInsightRowViewModel`), so Leptos tables render already prepared strings without inline metric formatting.
- [x] Slice 23: diagnostics card state badge and newest-indexed summary moved to core (`SearchDiagnosticsLabels`, `SearchDiagnosticsCardViewModel`, `build_search_diagnostics_card_view_model`), so Leptos card only passes host-provided labels and renders the prepared model.
- [x] Slice 24: render-ready values lagging diagnostics table rows moved to core (`LaggingSearchDocumentRowViewModel`, `build_lagging_search_document_row_view_models`), so Leptos table renders source/status label and lag as prepared strings.
- [x] Slice 25: render-ready values consistency diagnostics table rows moved to core (`SearchConsistencyIssueLabels`, `SearchConsistencyIssueRowViewModel`, `build_search_consistency_issue_row_view_models`), so Leptos table only passes host-provided labels and renders prepared issue/source/indexed fields.
- [x] Slice 26: admin native/GraphQL calls routed through module-owned `transport/` facade; Leptos admin calls `transport::*`, and `api.rs` remains the transport adapter implementation with the previous fallback contract.
- [x] Slice 27: admin/storefront render layers extracted to explicit `ui/leptos.rs` adapters; crate roots became wiring-only (`core` + `transport` + `ui`) and publicly re-export `SearchAdmin`/`SearchView` without changing GraphQL selected path or native server functions.
- [x] Slice 28: dictionaries table rows moved to core (`SearchSynonymRowViewModel`, `SearchStopWordRowViewModel`, `SearchQueryRuleRowViewModel`), so Leptos adapter no longer formats synonyms summary, pinned position and document/source path inline.
- [x] Slice 29: dictionaries mutation request construction moved to core (`SearchSynonymMutationRequest`, `SearchStopWordMutationRequest`, `SearchPinRuleMutationRequest`), so Leptos forms no longer parse CSV/pinned position and do not assemble transport payloads inline.
- [x] Slice 30: storefront results render-ready presentation moved to core (`SearchResultsLabels`, `SearchResultItemViewModel`, `SearchResultsViewModel`), so Leptos results adapter renders prepared summary/preset/locale/item fields.
- [x] Slice 31: storefront transport facade split into `transport/native_server_adapter.rs` and `transport/graphql_adapter.rs`; `transport/mod.rs` owns build-profile-selected orchestration for search, suggestions, presets and click tracking, and legacy `storefront/src/api.rs` is removed.
- [x] Slice 32: storefront suggestions presentation moved to core (`SearchSuggestionsLabels`, `SearchSuggestionNavigation`, `SearchSuggestionItemViewModel`, `build_search_suggestion_view_models`), so Leptos adapter receives ready kind/action labels and executes core-owned navigation target without inline document-vs-query branching.
- [x] Slice 33: storefront filter preset chips moved to core (`SearchPresetChipViewModel`, `build_search_preset_chip_view_models`, `preset_chip_class`), so Leptos adapter renders ready labels/state and does not store selected/idle class strings or next-selection policy inline.
- [x] Slice 34: storefront facet cards moved to core (`SearchFacetGroupViewModel`, `SearchFacetBucketViewModel`, `build_search_facet_view_models`), so Leptos adapter renders ready facet display names and bucket labels without inline formatting.
- [x] Slice 35: storefront result actions moved to core (`SearchResultActionViewModel`, `build_search_result_action_view_model`), so Leptos adapter renders prepared no-target/open-link states and only executes click tracking/navigation.
- [x] Slice 36: storefront empty states and feature cards moved to core (`SearchEmptyStateViewModel`, `SearchFeatureCardViewModel`, `build_search_empty_state_view_model`, `build_search_results_feature_cards`), so Leptos adapter renders ready title/body models without local presentation ownership.
- [x] Slice 37: storefront results header moved to core (`SearchResultsHeaderViewModel`), so Leptos adapter renders ready query label/query/summary/preset/locale fields without local header presentation assembly.
- [x] Slice 38: storefront search fetch request policy moved to core (`StorefrontSearchFetchRequest`, `search_preview_filters_from_route`, `build_storefront_search_fetch_request`), so Leptos resource executes the prepared request and does not own blank-query skip/query trim/preset normalization/filter payload mapping.
- [x] Slice 39: storefront search route-query policy moved to core (`StorefrontSearchRouteIntent`, `build_storefront_search_route_intent`), so Leptos navigation adapter applies the ready `q`/`preset` set/delete intent without inline normalization policy.
- [x] Slice 40: storefront suggestions request policy moved to core (`DEFAULT_SUGGESTION_MIN_LEN`, `StorefrontSuggestionFetchRequest`, `build_storefront_suggestion_fetch_request`), so Leptos suggestions resource executes the prepared request and does not own autocomplete threshold/query trim policy.

- [x] Slice 41: parity/evidence hardening added fast boundary guardrail `scripts/verify/verify-search-ui-boundary.mjs` and fixture suite `scripts/verify/verify-search-ui-boundary.test.mjs`; aggregate `verify:ffa:ui:migration`/`test:verify:ffa:ui:migration` now run search UI boundary without compilation.
- [x] Slice 42: admin legacy `api.rs` removed; native/GraphQL selected-path adapter moved to `admin/src/transport/native_server_adapter.rs`, and `verify-search-ui-boundary.mjs` prohibits return of `admin/src/api.rs` and `mod api`.

- [x] Slice 43: Leptos admin/storefront transport DTOs synchronized with catalog search contract; native `#[server]` and GraphQL pass channel/category/attribute/sort inputs, and facet UI uses host-locale projection label with fallback to stable value.

- [x] Slice 44: Leptos admin playground, Leptos storefront route UI, Next admin and Next storefront received visible catalog filter/sort controls over the same `channelId`/`categoryIds`/`attributeFilters`/`sortAttributeCode` contract; storefront route keys remain typed `snake_case`, Next surfaces pass optional fields without empty category arrays, and `verify-search-ui-boundary.mjs` source-locks Leptos controls, route keys and Next GraphQL/UI parity.

- [x] Slice 45: Search UI received picker-ready host metadata contract: Leptos `SearchAdmin`/`SearchView` and Next admin/storefront packages accept optional category/attribute option lists, render datalist hints for category/attribute/sort controls and do not import `rustok-product`; `verify-search-ui-boundary.mjs` source-locks this boundary and prohibits direct product dependency in search UI packages.

- [x] Slice 46: Next admin host composition connected real product-owned metadata options for search playground: `apps/next-admin/src/app/dashboard/search/page.tsx` takes category ids and filterable/sortable attribute codes through `packages/rustok-product` GraphQL helpers, passes host effective locale via `getLocale` and graceful-degrades to empty hints without blocking search surface. Guardrail locks product-owned metadata helpers and host composition markers.

- [x] Slice 47: `rustok-product-admin` added public Leptos metadata helper and neutral category/attribute option DTOs for future host composition. Helper requires host effective locale, reuses build-profile-selected native/GraphQL product transport, and search boundary guardrail and fixture regression prohibit loss of owner-owned helper or direct import of product internals in search UI.

- [x] Slice 48: `apps/admin` added `SearchAdminComposition` and the generated search page renderer uses it instead of direct mount. Host passes locale/auth/tenant, considers tenant module enablement and connects public product/search DTOs; product helper got current-tenant native `#[server]` endpoint and parallel GraphQL selected path. Guardrail locks the entire flow, including absence of package-local locale fallback.

- [x] Slice 49: `rustok-product-storefront` added public-safe category/attribute option DTOs and build-profile-selected native/GraphQL `fetch_catalog_search_options`; GraphQL field `storefrontCatalogSearchOptions(locale: String!)` uses tenant/channel guards without admin permission. `apps/storefront::SearchStorefrontComposition` connected in the generated search renderer, considers product enablement and host locale. Search boundary guardrail and fixture regression lock the full storefront flow.

- [x] Slice 50: `apps/next-frontend` added host-owned search composition and product-owned storefront metadata helper. Registry render context passes route locale, tenant slug and enabled modules, `SearchSection` calls `fetchCatalogSearchOptions` only when `product` is enabled, and `SearchStorefrontPage` receives category/attribute options as host props. `verify-search-ui-boundary.mjs` and fixture suite lock Next storefront flow without package-local locale fallback.

- [x] FBA slice #1: provider metadata and neutral read ports (`SearchQueryPort`, `SearchSuggestionPort`) locked in `search-fba-registry.json`; static evidence matrix and `npm run verify:search:fba` check deadline/error/locale fallback markers without long compilation.

- [x] FBA slice #2: `SearchSuggestionPort` received in-process PostgreSQL implementation through `SearchSuggestionService`, registry/evidence updated with no-compile source-locked fallback smoke `search-runtime-fallback-smoke.json`; executable runtime smoke remains the next step before FBA status promotion.

- [x] FBA slice #4: executable no-compile runtime fallback smoke `scripts/verify/verify-search-fba-runtime-smoke.mjs` checks read deadline enforcement, context locale fallback, typed `PortError` mapping and embedded PostgreSQL fallback source markers for `SearchQueryPort`/`SearchSuggestionPort`; aggregated into `npm run verify:search:fba` without compilation.

- [x] FBA slice #5: runtime fallback smoke strengthened with registry/degraded-mode parity, explicit locale preservation, tenant payload preservation, source-lock markers for `pg_engine.rs`/`suggestions.rs` and fixture regression suite `scripts/verify/verify-search-fba-runtime-smoke.test.mjs`; added no-compile test script `npm run test:verify:search:fba`.

- [x] FBA slice #6: runtime contract smoke added no-compile check of real in-process provider order for `SearchQueryPort`/`SearchSuggestionPort`: shared read policy executes before locale fallback, embedded PostgreSQL execution and typed `PortError` mapping; registry, README, local plan and central readiness board synchronized, and `npm run verify:search:fba` / `npm run test:verify:search:fba` include the new guardrail.

- [x] FBA slice #7: runtime invocation trace added no-compile executable simulation for real `SearchQueryPort`/`SearchSuggestionPort` boundary semantics: read-policy denial short-circuits before provider execution, context locale fallback does not overwrite explicit locale, success path calls embedded PostgreSQL provider exactly once, and validation/not_found/external/unknown errors map to typed `PortError`; registry, README, local plan, central readiness board and `npm run verify:search:fba` / `npm run test:verify:search:fba` synchronized.
