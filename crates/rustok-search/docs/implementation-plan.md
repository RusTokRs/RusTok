# План реализации `rustok-search`

Статус: dedicated search module уже работает на PostgreSQL baseline; локальная
документация и runtime boundary приведены к единому формату.

## Execution checkpoint

- Current phase: phase_b_in_progress
- Last checkpoint: Phase B slice #37 перенёс storefront results header summary/query/locale presentation в `storefront/src/core.rs`; Leptos header теперь рендерит `SearchResultsHeaderViewModel`.
- Next step: Продолжить Phase B: вынести оставшиеся storefront shell/form presentation или dictionaries mutation feedback envelope/status state в core view-model, сохраняя разделённые native/GraphQL adapters без изменения public contract.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок и central readiness board.
- Last updated at (UTC): 2026-06-03T00:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs;
  - Phase B slices #17-18 extracted admin route-query update semantics and preview form/request normalization into `admin/src/core.rs`; native/GraphQL transport was not modified;
  - Phase B slice #19 promoted reusable UI text/CSV and route-query update semantics to `rustok-api`, consumed by `leptos-ui-routing` and search admin core;
  - Phase B slice #20 перенёс render-ready labels, summary/preset text и представление result item-ов admin preview в `admin/src/core.rs`, оставив `admin/src/lib.rs` Leptos render adapter без изменений transport;
  - Phase B slice #21 перенёс форматирование analytics summary card values в `SearchAnalyticsSummaryViewModel`, поэтому Leptos analytics panel больше не форматирует метрики inline;
  - Phase B slice #22 перенёс форматирование analytics query/intelligence table rows в core row view-models, сохранив transport/native+GraphQL paths без изменений;
  - Phase B slice #23 перенёс diagnostics card state badge и newest-indexed summary в core view-model, оставив Leptos слой только для i18n labels и render;
  - Phase B slice #24 перенёс форматирование lagging diagnostics table rows в core row view-model, сохранив transport/native+GraphQL paths без изменений;
  - Phase B slice #25 перенёс consistency diagnostics issue labels, badge classes, source/status labels и indexed fallback в core row view-model;
  - Phase B slice #26 добавил module-owned `admin/src/transport/` facade для bootstrap, preview, analytics, diagnostics, settings и dictionary operations; Leptos admin больше не вызывает `api::*` напрямую, а existing native/GraphQL fallback adapter остался в `admin/src/api.rs`;
  - Phase B slice #27 выделил `admin/src/ui/leptos.rs` и `storefront/src/ui/leptos.rs` как явные render adapters; `admin/src/lib.rs` и `storefront/src/lib.rs` теперь только объявляют слои и re-export публичных entry points, без изменения native/GraphQL transport contract;
  - Phase B slice #28 добавил `SearchSynonymRowViewModel`, `SearchStopWordRowViewModel` и `SearchQueryRuleRowViewModel` в `admin/src/core.rs`; Leptos dictionaries tables больше не форматируют synonyms summary, pinned position и document/source path inline;
  - Phase B slice #29 добавил `SearchSynonymMutationRequest`, `SearchStopWordMutationRequest` и `SearchPinRuleMutationRequest` builders в `admin/src/core.rs`; Leptos dictionaries forms теперь передают в transport core-owned request objects вместо inline parsing/request construction;
  - Phase B slice #30 добавил `SearchResultsLabels`, `SearchResultItemViewModel` и `SearchResultsViewModel` в `storefront/src/core.rs`; Leptos storefront results больше не форматируют summary/preset/locale/source/score/snippet inline.
  - Phase B slice #31 разделил storefront transport facade на `native_server_adapter` и `graphql_adapter`; native-first fallback orchestration теперь находится в `storefront/src/transport/mod.rs`, а raw `api.rs` предоставляет только adapter endpoints.
  - Phase B slice #32 добавил `SearchSuggestionsLabels`, `SearchSuggestionNavigation` и `SearchSuggestionItemViewModel` в `storefront/src/core.rs`; Leptos suggestions adapter больше не решает inline document-vs-query navigation или action/kind labels.
  - Phase B slice #33 добавил `SearchPresetChipViewModel`, preset chip class constants и builder в `storefront/src/core.rs`; Leptos preset chips больше не хранят selected/idle class strings и next-selection policy inline.
  - Phase B slice #34 добавил `SearchFacetGroupViewModel`, `SearchFacetBucketViewModel` и `build_search_facet_view_models` в `storefront/src/core.rs`; Leptos facet cards больше не форматируют facet names/bucket labels inline.
  - Phase B slice #35 добавил `SearchResultActionViewModel` и `build_search_result_action_view_model` в `storefront/src/core.rs`; Leptos result cards больше не решают no-target/open-link labels, href state или click-tracking position inline.
  - Phase B slice #36 добавил `SearchEmptyStateViewModel`, `SearchFeatureCardViewModel`, `build_search_empty_state_view_model` и `build_search_results_feature_cards` в `storefront/src/core.rs`; Leptos empty/feature cards больше не владеют title/body presentation objects.
  - Phase B slice #37 добавил `SearchResultsHeaderViewModel` в `storefront/src/core.rs`; Leptos results header больше не собирает query label, query string, summary, preset и locale presentation inline.
- Last verified at (UTC): 2026-06-03T00:00:00Z
- Owner: `rustok-search` module team

## Область работ

- удерживать `rustok-search` как отдельный core module для search UX и engine semantics;
- не смешивать search responsibilities с `rustok-index`;
- синхронизировать backend contract, admin/storefront surfaces и observability.

## Текущее состояние

- модуль уже владеет `search_documents`, analytics storage, словарями и query rules;
- PostgreSQL FTS и `pg_trgm` служат baseline engine contract;
- Leptos и Next admin surfaces уже подключены, storefront path существует на том же backend contract;
- rebuild, diagnostics, analytics и settings editor уже составляют рабочий операторский baseline.
- operator-plane contract теперь дополнительно удерживается через `xtask`: public exports, README markers и `docs/observability-runbook.md` не должны деградировать при дальнейших рефакторингах.
- boundary `index != search` дополнительно удерживается contract-проверкой в `xtask`, чтобы search surface не откатывался к index-owned runtime types.

## Этапы

### 1. Contract stability

- [x] зафиксировать boundary `index != search`;
- [x] удерживать PostgreSQL как baseline engine и settings-driven engine selection;
- [x] держать admin/storefront surfaces на едином backend contract;
- [x] Expand capability matrix and contract tests;
- [x] Finalize search-facing error catalog and validation policy;
- [ ] удерживать sync между runtime metadata, UI packages и diagnostics surfaces.

### 2. Product hardening

- [ ] довести richer sorting/profile controls и advanced storefront UX polish;
- [ ] развить retry/DLQ strategy для ingestion/rebuild pipeline;
- [ ] завершить admin dashboards и production-grade analytics presentation.

### 3. Connector expansion

- [ ] добавить внешние connector crates для Meilisearch, Typesense и Algolia;
- [ ] зафиксировать degraded-mode и fallback contract для optional engines;
- [ ] документировать health/schema-sync guarantees для connector path.

## Проверка

- `cargo xtask module validate search`
- `cargo xtask module test search`
- targeted tests для ingestion, ranking, diagnostics, dictionaries и storefront/admin query flows

## Правила обновления

1. При изменении search runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении search/index boundary синхронизировать ADR и related docs.
4. При изменении metadata, UI packages или engine selection contract синхронизировать `rustok-module.toml`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.


## FFA pilot migration tracker (rustok-search)

- [x] Slice 1 scope locked (single use-case): query/filter input normalization (`parse_csv`, `optional_text`).
- [x] Storefront surface updated.
- [x] Admin surface checked/updated for the same use-case.
- [x] GraphQL fallback parity confirmed (no contract regression): transport path not modified in this slice.
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
- [x] Slice 20: render-ready view-model admin preview panel перенесён в core (`SearchPreviewLabels`, `SearchPreviewViewModel`, `build_search_preview_view_model`), поэтому Leptos panel только рендерит подготовленные поля и click actions.
- [x] Slice 21: render-ready values analytics summary cards перенесены в core (`SearchAnalyticsSummaryViewModel`, `build_search_analytics_summary_view_model`), поэтому Leptos analytics panel передаёт в cards уже подготовленные строки.
- [x] Slice 22: render-ready values analytics query/intelligence table rows перенесены в core (`SearchAnalyticsQueryRowViewModel`, `SearchAnalyticsInsightRowViewModel`), поэтому Leptos tables рендерят уже подготовленные строки без inline metric formatting.
- [x] Slice 23: diagnostics card state badge и newest-indexed summary перенесены в core (`SearchDiagnosticsLabels`, `SearchDiagnosticsCardViewModel`, `build_search_diagnostics_card_view_model`), поэтому Leptos card только передаёт host-provided labels и рендерит подготовленную модель.
- [x] Slice 24: render-ready values lagging diagnostics table rows перенесены в core (`LaggingSearchDocumentRowViewModel`, `build_lagging_search_document_row_view_models`), поэтому Leptos table рендерит source/status label и lag как подготовленные строки.
- [x] Slice 25: render-ready values consistency diagnostics table rows перенесены в core (`SearchConsistencyIssueLabels`, `SearchConsistencyIssueRowViewModel`, `build_search_consistency_issue_row_view_models`), поэтому Leptos table только передаёт host-provided labels и рендерит подготовленные issue/source/indexed поля.
- [x] Slice 26: admin native/GraphQL calls заведены за module-owned `transport/` facade; Leptos admin вызывает `transport::*`, а `api.rs` остаётся transport adapter implementation с прежним fallback контрактом.
- [x] Slice 27: admin/storefront render layers вынесены в явные `ui/leptos.rs` adapters; crate roots стали wiring-only (`core` + `transport` + `ui`) и публично re-export `SearchAdmin`/`SearchView` без изменения GraphQL fallback или native server functions.
- [x] Slice 28: dictionaries table rows перенесены в core (`SearchSynonymRowViewModel`, `SearchStopWordRowViewModel`, `SearchQueryRuleRowViewModel`), поэтому Leptos adapter больше не форматирует synonyms summary, pinned position и document/source path inline.
- [x] Slice 29: dictionaries mutation request construction перенесён в core (`SearchSynonymMutationRequest`, `SearchStopWordMutationRequest`, `SearchPinRuleMutationRequest`), поэтому Leptos forms больше не парсят CSV/pinned position и не собирают transport payloads inline.
- [x] Slice 30: storefront results render-ready presentation перенесён в core (`SearchResultsLabels`, `SearchResultItemViewModel`, `SearchResultsViewModel`), поэтому Leptos results adapter рендерит подготовленные summary/preset/locale/item поля.
- [x] Slice 31: storefront transport facade разделён на `transport/native_server_adapter.rs` и `transport/graphql_adapter.rs`; `transport/mod.rs` владеет native-first fallback orchestration для search, suggestions, presets и click tracking, а `api.rs` оставлен raw adapter endpoint слоем.
- [x] Slice 32: storefront suggestions presentation перенесён в core (`SearchSuggestionsLabels`, `SearchSuggestionNavigation`, `SearchSuggestionItemViewModel`, `build_search_suggestion_view_models`), поэтому Leptos adapter получает готовые kind/action labels и исполняет core-owned navigation target без inline document-vs-query branching.
- [x] Slice 33: storefront filter preset chips перенесены в core (`SearchPresetChipViewModel`, `build_search_preset_chip_view_models`, `preset_chip_class`), поэтому Leptos adapter рендерит готовые labels/state и не хранит selected/idle class strings или next-selection policy inline.
- [x] Slice 34: storefront facet cards перенесены в core (`SearchFacetGroupViewModel`, `SearchFacetBucketViewModel`, `build_search_facet_view_models`), поэтому Leptos adapter рендерит готовые facet display names and bucket labels без inline formatting.
- [x] Slice 35: storefront result actions перенесены в core (`SearchResultActionViewModel`, `build_search_result_action_view_model`), поэтому Leptos adapter рендерит prepared no-target/open-link states и только исполняет click tracking/navigation.
- [x] Slice 36: storefront empty states and feature cards перенесены в core (`SearchEmptyStateViewModel`, `SearchFeatureCardViewModel`, `build_search_empty_state_view_model`, `build_search_results_feature_cards`), поэтому Leptos adapter рендерит готовые title/body models без локального presentation ownership.
- [x] Slice 37: storefront results header перенесён в core (`SearchResultsHeaderViewModel`), поэтому Leptos adapter рендерит готовые query label/query/summary/preset/locale fields без локальной header presentation сборки.
