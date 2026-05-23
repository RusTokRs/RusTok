# План реализации `rustok-pages`

Статус: pages-owned storage и visual builder contract уже зафиксированы; модуль
удерживается в режиме steady-state hardening и rollout polish.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Закрыть sync-точки с `docs/modules/tiptap-page-builder-implementation-plan.md` и `docs/research/flutter.md` перед расширением mobile page-builder surfaces.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок и явно фиксировать, какие шаги Flutter/mobile ещё заблокированы текущим pages backend/parity статусом.
- Last updated at (UTC): 2026-05-23T00:00:00Z

## Область работ

- удерживать `rustok-pages` как владельца page, block и menu runtime contract;
- синхронизировать visual builder semantics, visibility rules и local docs;
- не допускать возврата page read/write paths на shared storage.

## Текущее состояние

- pages, page bodies, blocks и menus уже работают на module-owned persistence;
- GraphQL/REST adapters и Leptos admin/storefront packages уже живут внутри модуля;
- `grapesjs_v1` зафиксирован как canonical visual page-builder write-path;
- visibility contract уже использует typed relation `page_channel_visibility`.

## Этапы

### 1. Contract stability

- [x] закрыть storage split для pages, blocks и menus;
- [x] зафиксировать builder contract `markdown | rt_json_v1 | grapesjs_v1`;
- [x] удерживать compatibility surface для legacy block-driven pages;
- [ ] удерживать sync между runtime contracts, UI packages и module metadata;
- [ ] контрактные тесты покрывают все публичные use-case для уже поставленных pages runtime surfaces.

### 2. Product hardening

- [ ] удерживать GraphQL и REST surfaces синхронизированными при изменении page builder flows;
- [ ] развивать page/menu observability и write-path metrics при реальном operational pressure;
- [ ] документировать policy для authenticated/admin bypass и stricter visibility invariants, если она меняется.

### 3. Operability

- [ ] покрывать page/block/menu lifecycle targeted integration tests;
- [ ] документировать новые runtime guarantees одновременно с изменением visual builder и visibility contract;
- [ ] синхронизировать local docs, README и central references при изменении module boundary.

## Проверка

- `cargo xtask module validate pages`
- `cargo xtask module test pages`
- targeted tests для CRUD, blocks reorder, menus, builder round-trip и channel visibility

## Правила обновления

1. При изменении pages runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении dependency graph, UI wiring или visibility semantics синхронизировать `rustok-module.toml`.
4. При изменении shared rich-text expectations обновлять также связанные docs в `rustok-content`.
5. При изменении page-builder contract синхронно обновлять dependency-notes в `docs/modules/tiptap-page-builder-implementation-plan.md` и `docs/research/flutter.md`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.


## FFA pilot migration tracker (rustok-pages)

- [x] Slice 1: storefront selected-page core extraction (`selected_page_title/slug/effective_locale`, `summarize_page_content`).
- [x] Slice 2: admin form helper extraction (`slugify`, `parse_channel_slugs`, `error_with_context`).
- [x] Storefront + admin surfaces updated for selected slices.
- [x] `cargo xtask module validate pages` passed.
- [ ] `cargo xtask module test pages` full run evidence attached.
- [x] Double documentation verification completed.
- [x] Slice 3: admin status badge class mapping moved to core (`status_badge_class`).
- [x] Slice 4: admin busy-key composition moved to core (`busy_key_with_id`, `busy_key_for_save`).
- [x] Slice 5: admin edit-form seed mapping moved to core (`edit_form_seed_from_page`).
- [x] Slice 6: admin list-load error rendering switched to core error composition (`error_with_context`).
- [x] Slice 7: admin status badge css composition moved to core (`status_badge_css`).
- [x] Slice 8: admin busy-key action matching moved to core (`busy_key_matches_action`).


## Перепроверка после slices #2-#8

- [x] Code/docs consistency check completed for `rustok-pages/admin` and `rustok-pages/storefront`.
- [x] Tracker wording synced with actual `core` extraction state.
- [x] No transport-contract changes introduced (`native #[server]` + GraphQL fallback preserved).

- [x] Slice 9: storefront raw-format body summary rendering moved to core (`raw_body_format_summary`).

- [x] Slice 10: pages tracker synchronized after double documentation verification completion.
