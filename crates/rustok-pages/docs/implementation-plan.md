# План реализации `rustok-pages`

Статус: pages-owned storage и visual builder contract уже зафиксированы; модуль
переводится в FBA-consumer режим для visual builder capability layer и удерживается
в steady-state hardening + rollout polish.

## Execution checkpoint

- Current phase: phase_b_operability_rollout_guardrail
- Last checkpoint: B4 maintenance slice зафиксировал no-compile guardrail для legacy blocks read/bridge режима (`verify-page-builder-pages-legacy-bridge.mjs`): create/import blocks остаются bridge surface, visual builder body writes не удаляют blocks, update DTO не расширяет block write surface, admin/storefront показывают read-only compatibility evidence; устранён drift в `registry.md`, декомпозирован `Quality backlog` локального плана и успешно пройдены верификационные гейты.
- Next step: Провести реальный control-plane Wave 0 dry-run на internal tenant и заменить синтетический пакет фактическими before/after snapshots; затем оставить pages в maintenance mode до следующего явного builder/FBA среза. Для FFA boundary evidence использовать быстрый `verify-pages-ui-boundary.mjs` guardrail; для FBA rollout policy использовать `npm run verify:page-builder:consumer:pages`.
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
- Last updated at (UTC): 2026-05-29T00:00:00Z
- Last updated at (UTC): 2026-06-01T00:00:00Z
- Last updated at (UTC): 2026-06-01T01:00:00Z
- Last updated at (UTC): 2026-06-01T02:00:00Z
- Last updated at (UTC): 2026-06-01T03:00:00Z
- Last updated at (UTC): 2026-06-01T04:00:00Z
- Last updated at (UTC): 2026-06-01T04:30:00Z
- Last updated at (UTC): 2026-06-01T11:45:00Z
- Last updated at (UTC): 2026-06-07T00:00:00Z
- Last updated at (UTC): 2026-06-13T00:00:00Z
- Last updated at (UTC): 2026-06-14T00:00:00Z
- Last updated at (UTC): 2026-06-14T12:00:00Z
- Latest maintenance update: Leptos admin package now exposes capability surfaces `preview/tree/properties/publish` for `grapesjs_v1` and keeps legacy `blocks` compatibility visible in the same write-path.
- Latest maintenance update: зафиксирован typed builder error catalog parity (`validation/sanitize/runtime/feature-disabled`) для admin UI + service/runtime с опорой на `WritePathIssueKind`, `PagesError::FeatureDisabled`, manifest/registry binding и `verify-page-builder-error-catalog-binding.mjs`.
- Latest maintenance update: create-page draft normalization теперь собирается в `admin/src/core.rs` и переиспользует `rustok-api::normalize_ui_text` / `parse_ui_csv`, а Leptos слой остаётся thin bind/render adapter.
- Latest maintenance update: admin UI получил явный FFA split `core` + `transport` + `ui/leptos`; GraphQL operations остаются в `admin/src/api.rs`, а render/effect код вызывает только facade из `admin/src/transport/`.
- Latest FFA update: storefront UI получил matching split `core` + `transport` + `ui/leptos`; crate root re-export-ит `PagesView`, Leptos adapter вызывает только `storefront/src/transport.rs`, а native/GraphQL transport contract не менялся. Быстрый guardrail `scripts/verify/verify-pages-ui-boundary.mjs` закрепляет admin/storefront boundary без full-workspace compile.
- Latest FBA rollout update: manifest `fba.builder_consumer.rollout_policy` теперь закрепляет control-plane audit trail, mandatory before/after tenant snapshots, keep/rollback decision, owner sign-off, rollback target <= 10 минут без redeploy, SLO rollback triggers и pilot smoke `preview -> properties -> publish(dry)`; `verify-page-builder-consumer-readiness.mjs pages` проверяет эти markers без компиляции.
- Latest legacy bridge update: `verify-page-builder-pages-legacy-bridge.mjs` добавлен в FBA baseline и фиксирует read/bridge semantics для legacy `blocks`: import/create разрешён, visual-builder body writes не удаляют blocks, update surface не получает новый block write contract, admin/storefront показывают compatibility evidence.

- PB-FBA-1 platform sync note: central plan `docs/modules/tiptap-page-builder-implementation-plan.md` now содержит delivery slices и exit criteria для Wave 0 hand-off; pages track должен обновляться синхронно по dependency notes.
- PB-FBA-1 execution note: sync с central section `8.5 Execution backlog` принят как active queue (`PB-FBA-1A..1D`, фокус Week1=P0/P1, Week2=P2/P3).
- PB-FBA-1A update: `consumer_min_version = "1.0"` добавлен в `fba.builder_consumer`, а machine-readable registry `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` теперь проверяется через `verify-page-builder-contract-registry.mjs` и aggregate baseline gate.
- PB-FBA-1B host update: `pages_builder_fallback_*` gate покрывает все baseline-профили (`all_on`, `publish_off`, `preview_off`, `builder_off`) на service boundary и admin/storefront host helpers: read/list остаются стабильными, disabled capabilities возвращают typed `FeatureDisabled`, storefront render не требует builder capability.
- PB-FBA-1B catalog update: `fba.builder_consumer.error_catalog`, `error_codes` и `degraded_mode_errors` синхронизированы с provider metadata, FBA registry и runtime constants; aggregate baseline gate теперь включает anti-drift проверку error-catalog binding.
- PB-FBA-1B Next parity update: `apps/next-admin` save-flow отображает тот же typed catalog (`validation/sanitize/runtime/feature-disabled`) и operator guidance для `FEATURE_DISABLED`; baseline gate включает static parity-check для Next Admin.
- PB-FBA-1B Leptos parity update: module-owned Leptos admin показывает localized operator guidance для `validation/sanitize/runtime/feature-disabled`; baseline gate включает static parity-check для `rustok-pages-admin`.
- PB-FBA-1B Flutter parity update: `rustok_mobile/packages/app_core` содержит shared mapper для того же typed catalog и `FEATURE_DISABLED` guidance; baseline gate включает static parity-check для Flutter app-core.
- PB-FBA-1D synthetic observability update: Wave 0 dry-run packet теперь содержит baseline metrics, pilot SLO thresholds/evaluation и 2 correlation trace samples (`builder_write -> pages_publish -> storefront_read`); `verify-page-builder-wave-evidence-packet.mjs` блокирует threshold drift, placeholder traces, missing spans и неполный correlation path. Фактические tenant metrics/traces остаются Wave hand-off evidence.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress` (consumer baseline для `rustok-page-builder`; remote runtime profile ещё не включён)
- Structural shape: `core_transport_ui`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board;
  - FBA consumer metadata синхронизирована с `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`, `rustok-module.toml` и baseline gate;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs;
  - FFA maintenance slice: create-page draft normalization, channel slug CSV parsing and route text checks переиспользуют shared UI helpers из `rustok-api` без изменения native/GraphQL транспорта;
  - FFA admin slice: Leptos render/effect adapter живёт в `admin/src/ui/leptos.rs`, transport facade — в `admin/src/transport/`, GraphQL adapter — в `admin/src/api.rs`; внешний GraphQL contract не менялся;
  - FFA storefront slice: Leptos render/bind adapter живёт в `storefront/src/ui/leptos.rs`, crate root только wires modules/re-export `PagesView`, transport facade — в `storefront/src/transport.rs`, native/GraphQL adapter — в `storefront/src/api.rs`; fast boundary guardrail `scripts/verify/verify-pages-ui-boundary.mjs` фиксирует admin/storefront split, Leptos-free core и docs sync.
- Last verified at (UTC): 2026-06-14T00:00:00Z
- Owner: `rustok-pages` module team

## PB-FBA immediate sprint (продолжение page builder разработки)

### Sprint goal

Перевести `rustok-pages` из статуса “handshake in progress” в проверяемый FBA-consumer baseline, который можно масштабировать на следующие модули по тому же шаблону.

### Sprint scope (must-have)

- [x] Typed fallback matrix: `builder_off`, `preview_off`, `publish_off` с ожидаемыми runtime/error outcomes.
- [x] Unified builder error catalog для `validation/sanitize/runtime/feature-disabled` без расхождения между GraphQL, `#[server]` и UI adapters.
- [x] CI fallback gate для профилей `all_on`, `publish_off`, `preview_off`, `builder_off`: provider runtime gate и `rustok-pages` service/admin/storefront consumer fallback gate подключены к baseline-проверке.
- [x] Contract freeze anti-drift: `builder_contract_version`, `consumer_min_version`, capability set и fallback profile names зафиксированы в machine-readable registry и проверяются aggregate baseline gate.

### Fallback matrix (admin/list/read/publish snapshots)

Эта матрица является consumer-side snapshot для `rustok-pages` и должна совпадать с provider matrix в `rustok-page-builder::rollout`. Read/list/menu paths остаются owned by pages и не должны зависеть от доступности builder capability endpoint.

| Профиль | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Disabled capabilities |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview`, `publish` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `preview`, `tree`, `properties`, `publish` |

Операционные заметки:

1. `builder_off` не отключает pages-owned list/read/menu runtime; admin visual path обязан показать read-only fallback вместо 5xx.
2. `publish_off` возвращает typed `feature-disabled`/`typed_feature_disabled_error` только на builder publish path; legacy/direct read paths остаются стабильными.
3. `preview_off` скрывает или блокирует preview capability, но не должен запрещать properties/tree чтение, если `builder.properties.enabled=true`.

- [x] Wave 0 evidence template: flags snapshot + smoke output + observability snapshot + keep/rollback decision (`crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json`).
- [x] Синтетический Wave 0 dry-run packet для всех baseline-профилей: `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json` (проверяет форму, fallback-семантику, baseline metrics, SLO thresholds/evaluation и минимум 2 correlation trace samples; не заменяет фактическое tenant evidence).

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
- [x] Legacy blocks path работает в режиме read/bridge без расширения write surface (`verify-page-builder-pages-legacy-bridge.mjs`).
- [x] Переключение tenant flags не требует redeploy и оставляет list/read surfaces доступными.

### Tenant switch procedure (operational checklist)

Manifest source of truth: `fba.builder_consumer.rollout_policy` in `rustok-module.toml`.

1. Capture `before` snapshot по flags и module health в `control_plane_builder_wave_audit`.
2. Apply change-set (`builder.enabled`, `builder.preview`, `builder.properties`, `builder.publish`).
3. Run targeted smoke (`list -> open -> preview -> save-draft -> publish-dry`) и обязательный pilot smoke `preview -> properties -> publish(dry)`.
4. Validate logs/metrics (`sanitize`, `runtime`, `publish_latency`).
5. Capture `after` snapshot + decision note (`keep/rollback`) + owner sign-off in the same audit trail.

Rollback trigger:

- runtime errors выше alert threshold;
- publish latency p95 выше целевого SLO в течение 10 минут;
- sanitize failures выше alert threshold;
- storefront read regression на published pages.

Rollback target: переключение tenant flags назад должно занимать <= 10 минут и не требует redeploy `pages` runtime; pages-owned list/read/menu surfaces остаются доступными во всех degraded builder-профилях.

## Этапы

### 1. Contract stability

- [x] закрыть storage split для pages, blocks и menus;
- [x] зафиксировать builder contract `markdown | rt_json_v1 | grapesjs_v1`;
- [x] удерживать compatibility surface для legacy block-driven pages;
- [x] удерживать sync между runtime contracts, UI packages и module metadata;
- [ ] контрактные тесты покрывают все публичные use-case для уже поставленных pages runtime surfaces.
- [x] зафиксировать в runtime metadata, что builder capability layer является внешним provider-контуром.

### 2. Product hardening

- [ ] удерживать GraphQL и REST surfaces синхронизированными при изменении page builder flows;
- [ ] развивать page/menu observability и write-path metrics при реальном operational pressure;
- [ ] документировать policy для authenticated/admin bypass и stricter visibility invariants, если она меняется.
- [x] описать tenant-level toggle policy для capability surfaces (`builder.preview/tree/properties/publish`) без деградации core pages runtime.

### 3. Operability

- [ ] покрывать page/block/menu lifecycle targeted integration tests;
- [ ] документировать новые runtime guarantees одновременно с изменением visual builder и visibility contract;
- [x] синхронизировать local docs, README и central references при изменении module boundary.
- [x] добавить FBA runbook: partial disable capability layer + fallback behavior для admin/storefront paths.

## FBA execution backlog (`pages` как consumer reference builder-модуля)

### B1. Contract & metadata hardening

- [x] Обновить runtime metadata/manifest: явно указать внешний `builder capability-provider` и поддерживаемые capability surfaces (`preview/tree/properties/publish`) — см. `rustok-module.toml` (`dependencies.page_builder`, `fba.builder_consumer`).
- [x] Добавить contract-version marker для anti-drift проверок между `pages`, Next/Leptos adapters и reference builder (`contract_version = "1.0"` в metadata consumer/provider link).
- [x] Добавить `consumer_min_version = "1.0"` и синхронизировать machine-readable registry `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` с manifest provider/consumer contract values.
- [x] Зафиксировать machine-readable degraded modes (`builder_disabled`, `publish_disabled`, `preview_disabled`) в `fba.builder_consumer.degraded_modes`.

### B2. Fallback & error semantics

- [x] Закрепить единый typed error catalog для builder-related runtime ошибок (`validation/sanitize/runtime/feature-disabled`) и связать его с `degraded_modes` через machine-readable manifest/registry gate.
- [x] Добавить fallback snapshots в docs для admin/list/read/publish surfaces.
- [x] Убедиться, что baseline-профили `all_on`, `publish_off`, `preview_off`, `builder_off` не ломают page read/list/menu paths на service fallback gate и host-level admin/storefront helper checks; Next Admin, Leptos и Flutter app-core typed-error parity зафиксированы; runtime device-level evidence остаётся в Wave hand-off.

### B3. Operability & rollout

- [x] Привязать tenant switch checklist к control-plane audit trail (before/after snapshots + decision) через `fba.builder_consumer.rollout_policy.audit_trail`.
- [x] Синхронизировать rollback triggers с platform SLO policy (p95 publish, runtime error-rate, sanitize failures) в manifest rollout policy.
- [x] Добавить runbook-note для pilot-tenants: обязательный smoke `preview -> properties -> publish(dry)`.

### B4. Verification gates

- [x] Включить fallback regression checks в `cargo xtask module test pages` (или эквивалентный CI gate): `verify-page-builder-fba-baseline.mjs` запускает provider runtime gate, registry anti-drift gate, error-catalog binding gate, Next Admin parity gate, Leptos admin parity gate, Flutter parity gate, Wave evidence-template gate, gate синтетического evidence packet, `rustok-pages` service/admin/storefront fallback gates по всем четырём baseline-профилям и no-compile legacy blocks read/bridge guardrail `verify-page-builder-pages-legacy-bridge.mjs`.
- [x] Добавить targeted integration checks для `all_on`, `publish_off`, `preview_off`, `builder_off` на уровне `pages` service/transport boundary (`pages_builder_fallback_*` checks).
- [x] Зафиксировать evidence-template для Wave hand-off (platform + pages owner approval): `crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json` + `verify-page-builder-wave-evidence-template.mjs`.

## Wave 0 execution checklist (операционный минимум для `pages`)

### C1. Toggle profiles (обязательно)

- [x] `all_on`: `builder.enabled=true`, `preview/properties/publish=true` (service + admin/storefront host fallback gate).
- [x] `publish_off`: `builder.publish.enabled=false`, publish-path возвращает typed `feature-disabled` error, read/list paths стабильны.
- [x] `preview_off`: preview capability недоступен, read/list surfaces не деградируют (service + admin/storefront host fallback gate).
- [x] `builder_off`: service read/list paths стабильны, publish-path возвращает typed `feature-disabled`; UI read-only fallback остаётся Wave evidence.

### C2. Evidence package для каждого профиля

- [~] before/after snapshot флагов и module health: синтетический dry-run packet зафиксирован; фактические tenant snapshots ещё ожидаются.
- [~] smoke output: `list -> open -> preview -> save-draft -> publish-dry` (синтетические ожидаемые outcomes зафиксированы; фактический control-plane smoke output ещё ожидается).
- [~] observability snapshot: `sanitize`, `runtime`, `publish_latency` (синтетические placeholders зафиксированы; фактические метрики ещё ожидаются).
- [~] решение `keep/rollback` + owner подпись в control-plane audit trail (синтетическое решение `keep` зафиксировано; фактический owner sign-off ещё ожидается).

### C3. Exit criteria для Wave 1

- [x] service-level fallback regression checks и admin/storefront host-helper static checks зелёные на актуальном коммите; Next/Flutter typed error parity ещё требуется для Wave 1.
- [ ] нет RBAC regression для editor/moderator/admin в builder-related сценариях.
- [~] подтверждён rollback execution <= 10 минут без redeploy `pages` runtime: manifest target зафиксирован, фактическое tenant evidence ожидается в реальном Wave 0 dry-run.

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

### Тесты (Tests)
- [ ] Добавить интеграционные тесты для degraded-режимов (publish_off, preview_off, builder_off) на уровне сервис-границ rustok-pages.
- [ ] Написать автотесты для проверки корректности отображения ошибок маппинга FeatureDisabled в Leptos и Next.js UI-компонентах.
- [ ] Реализовать тесты на обход visibility-ограничений (channel visibility, page channel visibility) для ролей с административными правами.

### Документация (Documentation)
- [ ] Провести ревизию README.md и docs/README.md на соответствие реальной схеме БД (таблицы pages, page_translations, page_bodies, page_blocks, menus, menu_translations, menu_items, menu_item_translations).
- [ ] Документировать поведение degraded-режимов и toggle policy для tenant-флагов без необходимости передеплоя.
- [ ] Зафиксировать в README архитектурное разделение ответственности между runtime-контуром pages и внешним провайдером page-builder.

### Инструменты и гейты верификации (Verification Gates)
- [ ] Интегрировать скрипт verify-page-builder-pages-legacy-bridge.mjs в хуки pre-commit.
- [ ] Внедрить запуск verify-pages-ui-boundary.mjs в pre-push пайплайн.
- [ ] Реализовать автоматическую сверку схемы ошибок в CI между backend-слоем и UI-компонентами для предотвращения drift-а типов ошибок.


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
- [x] Slice 18: storefront Leptos render/bind code moved to explicit `storefront/src/ui/leptos.rs` adapter; crate root now only wires modules and re-exports `PagesView`.


## Phase B pilot closure (rustok-pages)

- [x] Core extraction slices for admin/storefront completed for planned helper scope.
- [x] Explicit `ui/leptos.rs` adapters exist for both admin and storefront surfaces while dual-path transport remains unchanged.
- [x] Module validation evidence attached (`cargo xtask module validate pages`).
- [x] Module test evidence attached (`cargo xtask module test pages`).
- [x] Double documentation verification completed and synced in central tracker.
- [x] Ready to move primary focus to next module wave while keeping pages in maintenance mode.


## B3 operability rollout guardrail (2026-06-13)

- Manifest rollout policy закрепляет `control_plane_builder_wave_audit` как обязательный audit trail для before/after snapshots, keep/rollback decision и owner sign-off.
- Pilot tenants обязаны выполнять smoke `preview -> properties -> publish(dry)` дополнительно к общему `list -> open -> preview -> save-draft -> publish-dry`.
- Rollback triggers синхронизированы с platform SLO policy: runtime error-rate, publish p95 за 10 минут, sanitize failures и storefront published-read regression.
- Rollback target зафиксирован как <= 10 минут без redeploy `pages` runtime; core pages-owned list/read/menu paths остаются стабильными при disabled builder capabilities.
- Verification: `npm run verify:page-builder:consumer:pages` проверяет наличие rollout policy markers без Cargo-компиляции.
