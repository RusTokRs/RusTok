# План реализации `rustok-search`

Статус: dedicated search module уже работает на PostgreSQL baseline; локальная
документация и runtime boundary приведены к единому формату.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Phase B pilot slice #16 moved admin relevance editor merge/array validation into `admin/src/core.rs` without changing native/GraphQL transport.
- Next step: Continue Phase B with the next smallest search UI/state helper extraction while preserving dual-path parity.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-28T00:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs.
- Last verified at (UTC): 2026-05-28T00:00:00Z
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
