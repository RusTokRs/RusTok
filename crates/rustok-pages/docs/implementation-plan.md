# План реализации `rustok-pages`

Статус: pages-owned storage и visual builder contract уже зафиксированы; модуль
переводится в FBA-consumer режим для visual builder capability layer и удерживается
в steady-state hardening + rollout polish.

## Execution checkpoint

- Current phase: phase_b_closed
- Last checkpoint: Phase B pilot closure зафиксирован (core extraction + validate/test + docs double-check).
- Next step: Выполнить PB-FBA-1 (typed fallback matrix + error catalog parity + anti-drift CI checks) и подготовить Wave 0 evidence package для tenant toggles.
- Open blockers: None.
- Hand-off notes for next agent:
  1. Перед любыми изменениями pages сначала сверить `docs/research/dioxus-ffa-pilot-connectivity-map.md` и этот файл; не открывать новый slice без явной цели в трекере.
  2. Для кода ориентироваться на текущий образец: Leptos UI = thin render/bind, formatting/parsing helpers = `core::*`, dual-path (`native #[server]` + GraphQL fallback) не менять.
  3. Если задача не про pages runtime contract, приоритет смещается на следующий модуль волны; в pages вносить только bugfix/contract-sync.
- Last updated at (UTC): 2026-05-24T00:40:00Z
- Last updated at (UTC): 2026-05-24T09:15:00Z
- Last updated at (UTC): 2026-05-24T10:05:00Z
- Last updated at (UTC): 2026-05-24T12:20:00Z
- Last updated at (UTC): 2026-05-25T11:10:00Z
- Latest maintenance update: Leptos admin package now exposes capability surfaces `preview/tree/properties/publish` for `grapesjs_v1` and keeps legacy `blocks` compatibility visible in the same write-path.
- Latest maintenance update: зафиксирован typed builder error catalog parity (`validation/sanitization/runtime/feature-disabled`) для admin UI + service/runtime с опорой на `WritePathIssueKind` и `PagesError::FeatureDisabled`.

- PB-FBA-1 platform sync note: central plan `docs/modules/tiptap-page-builder-implementation-plan.md` now содержит delivery slices и exit criteria для Wave 0 hand-off; pages track должен обновляться синхронно по dependency notes.
- PB-FBA-1 execution note: sync с central section `8.5 Execution backlog` принят как active queue (`PB-FBA-1A..1D`, фокус Week1=P0/P1, Week2=P2/P3).





## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs.
- Last verified at (UTC): 2026-05-24T00:00:00Z
- Owner: `rustok-pages` module team

## PB-FBA immediate sprint (продолжение page builder разработки)

### Sprint goal

Перевести `rustok-pages` из статуса “handshake in progress” в проверяемый FBA-consumer baseline, который можно масштабировать на следующие модули по тому же шаблону.

### Sprint scope (must-have)

- [ ] Typed fallback matrix: `builder_off`, `preview_off`, `publish_off` с ожидаемыми runtime/error outcomes.
- [x] Unified builder error catalog для `validation/sanitize/runtime/feature-disabled` без расхождения между GraphQL, `#[server]` и UI adapters.
- [ ] CI fallback gate для профилей `builder.enabled=false` и `builder.publish.enabled=false`.
- [ ] Wave 0 evidence template: flags snapshot + smoke output + observability snapshot + keep/rollback decision.

### Out of scope (for this sprint)

- Расширение visual editor функционала за пределы capability contract.
- Любой vendor-specific surface вне `grapesjs_v1`.
- Изменение ownership boundaries (pages runtime owner vs external builder capability provider).


## Область работ

- удерживать `rustok-pages` как владельца page, block и menu runtime contract;
- синхронизировать visual builder semantics как внешний FBA capability layer, visibility rules и local docs;
- не допускать возврата page read/write paths на shared storage.

## Текущее состояние

- pages, page bodies, blocks и menus уже работают на module-owned persistence;
- GraphQL/REST adapters и Leptos admin/storefront packages уже живут внутри модуля;
- `grapesjs_v1` зафиксирован как canonical visual page-builder write-path;
- visibility contract уже использует typed relation `page_channel_visibility`;
- write-path UX для page builder теперь использует единый паттерн ошибок `validation/sanitize/runtime` и contract-safe JSON handling для `body.contentJson`.

## FBA migration frame (`pages` как consumer reference builder-модуля)

- `rustok-pages` продолжает владеть page/menu lifecycle и publish pipeline.
- Builder-domain (`preview/tree/properties/publish`) рассматривается как внешний capability-provider.
- В module docs и runtime metadata фиксируется запрет на возврат к pages-local ownership визуального builder runtime.
- Legacy block-driven path удерживается как compatibility-bridge с явным sunset roadmap.


## Dedicated page-builder track (FBA hand-off scope)

### Scope now

- pages runtime остаётся owner для `page/menu/visibility/routing`.
- visual builder write-path работает через внешний capability-provider (`preview/tree/properties/publish`).
- module-level runbook обязан описывать degraded mode при отключении builder capability.

### Acceptance criteria for hand-off

- [x] Admin UI показывает понятный fallback-state при `builder.enabled=false`.
- [x] Storefront read-path не зависит от availability builder capability endpoint.
- [x] Publish endpoint корректно возвращает typed runtime error при `builder.publish.enabled=false`.
- [ ] Legacy blocks path работает в режиме read/bridge без расширения write surface.
- [x] Переключение tenant flags не требует redeploy и оставляет list/read surfaces доступными.

### Tenant switch procedure (operational checklist)

1. Capture `before` snapshot по flags и module health.
2. Apply change-set (`builder.enabled`, `builder.preview`, `builder.properties`, `builder.publish`).
3. Run targeted smoke (`list -> open -> preview -> save-draft -> publish-dry`).
4. Validate logs/metrics (`sanitize`, `runtime`, `publish_latency`).
5. Capture `after` snapshot + decision note (`keep/rollback`).

Rollback trigger:

- runtime errors выше alert threshold;
- publish latency p95 выше целевого SLO в течение 10 минут;
- storefront read regression на published pages.

## Этапы

### 1. Contract stability

- [x] закрыть storage split для pages, blocks и menus;
- [x] зафиксировать builder contract `markdown | rt_json_v1 | grapesjs_v1`;
- [x] удерживать compatibility surface для legacy block-driven pages;
- [ ] удерживать sync между runtime contracts, UI packages и module metadata;
- [ ] контрактные тесты покрывают все публичные use-case для уже поставленных pages runtime surfaces.
- [ ] зафиксировать в runtime metadata, что builder capability layer является внешним provider-контуром.

### 2. Product hardening

- [ ] удерживать GraphQL и REST surfaces синхронизированными при изменении page builder flows;
- [ ] развивать page/menu observability и write-path metrics при реальном operational pressure;
- [ ] документировать policy для authenticated/admin bypass и stricter visibility invariants, если она меняется.
- [ ] описать tenant-level toggle policy для capability surfaces (`builder.preview/tree/properties/publish`) без деградации core pages runtime.

### 3. Operability

- [ ] покрывать page/block/menu lifecycle targeted integration tests;
- [ ] документировать новые runtime guarantees одновременно с изменением visual builder и visibility contract;
- [ ] синхронизировать local docs, README и central references при изменении module boundary.
- [ ] добавить FBA runbook: partial disable capability layer + fallback behavior для admin/storefront paths.

## FBA execution backlog (`pages` как consumer reference builder-модуля)

### B1. Contract & metadata hardening

- [x] Обновить runtime metadata/manifest: явно указать внешний `builder capability-provider` и поддерживаемые capability surfaces (`preview/tree/properties/publish`) — см. `rustok-module.toml` (`dependencies.page_builder`, `fba.builder_consumer`).
- [x] Добавить contract-version marker для anti-drift проверок между `pages`, Next/Leptos adapters и reference builder (`contract_version = "1.0"` в metadata consumer/provider link).
- [x] Зафиксировать machine-readable degraded modes (`builder_disabled`, `publish_disabled`, `preview_disabled`) в `fba.builder_consumer.degraded_modes`.

### B2. Fallback & error semantics

- [x] Закрепить единый typed error catalog для builder-related runtime ошибок (`validation/sanitize/runtime/feature-disabled`).
- [ ] Добавить fallback snapshots в docs для admin/list/read/publish surfaces.
- [ ] Убедиться, что partial disable не ломает page read/list/menu paths в storefront/admin.

### B3. Operability & rollout

- [ ] Привязать tenant switch checklist к control-plane audit trail (before/after snapshots + decision).
- [ ] Синхронизировать rollback triggers с platform SLO policy (p95 publish, runtime error-rate, sanitize failures).
- [ ] Добавить runbook-note для pilot-tenants: обязательный smoke `preview -> properties -> publish(dry)`.

### B4. Verification gates

- [ ] Включить fallback regression checks в `cargo xtask module test pages` (или эквивалентный CI gate).
- [ ] Добавить targeted integration checks для `builder.publish.enabled=false` и `builder.enabled=false`.
- [ ] Зафиксировать evidence-template для Wave hand-off (platform + pages owner approval).

## Wave 0 execution checklist (операционный минимум для `pages`)

### C1. Toggle profiles (обязательно)

- [ ] `all_on`: `builder.enabled=true`, `preview/properties/publish=true`.
- [ ] `publish_off`: `builder.publish.enabled=false`, publish-path возвращает typed `feature-disabled` error.
- [ ] `preview_off`: preview capability недоступен, read/list surfaces не деградируют.
- [ ] `builder_off`: admin visual path в read-only fallback, storefront read-path стабилен.

### C2. Evidence package для каждого профиля

- [ ] before/after snapshot флагов и module health.
- [ ] smoke output: `list -> open -> preview -> save-draft -> publish-dry` (с ожидаемым результатом для профиля).
- [ ] observability snapshot: `sanitize`, `runtime`, `publish_latency`.
- [ ] решение `keep/rollback` + owner подпись в control-plane audit trail.

### C3. Exit criteria для Wave 1

- [ ] fallback regression checks зелёные в CI на актуальном коммите.
- [ ] нет RBAC regression для editor/moderator/admin в builder-related сценариях.
- [ ] подтверждён rollback execution <= 10 минут без redeploy `pages` runtime.

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
- [x] `cargo xtask module test pages` full run evidence attached.
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
- [x] Slice 11: admin reset-form defaults delegated to core (`empty_edit_form_seed`).
- [x] Slice 12: admin table count-label placeholder rendering moved to core (`count_label`).
- [x] Slice 13: storefront published-pages total count placeholder rendering moved to core (`count_label`).
- [x] Slice 14: admin editing-banner `{id}` placeholder rendering moved to core (`label_with_id`).
- [x] Slice 15: storefront open-link label composition moved to core (`open_link_label`).
- [x] Slice 16: storefront label/value pair rendering moved to core (`label_value_pair`).
- [x] Slice 17: storefront core extraction cleanup after full module test evidence (unused import removal).


## Phase B pilot closure (rustok-pages)

- [x] Core extraction slices for admin/storefront completed for planned helper scope.
- [x] Module validation evidence attached (`cargo xtask module validate pages`).
- [x] Module test evidence attached (`cargo xtask module test pages`).
- [x] Double documentation verification completed and synced in central tracker.
- [x] Ready to move primary focus to next module wave while keeping pages in maintenance mode.
