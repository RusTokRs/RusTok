# Реестр implementation plans (crate-level)

Этот реестр — единая операционная точка для сопровождения implementation plans по crate-ам.
Используйте его как "single pane of glass": сначала обновляйте статус здесь, затем переходите в локальный план модуля.

## Области покрытия

Каждый implementation plan в crate должен включать два обязательных направления в одном документе:

- feature delivery (функциональные этапы),
- quality backlog (тесты, документация, DX и quality gates).

Отдельный второй план для quality **не нужен**: качество ведётся в том же `docs/implementation-plan.md` через отдельную секцию/чеклист.

## Как работать с реестром

1. Найдите запись, на которую указывает `next_plan_id` в `Cycle state`.
2. Откройте linked plan и выполните ограниченный по времени итерационный шаг (рекомендуется 30–60 минут или 1 PR).
3. Внутри итерации обязательно сделать оба шага:
   - синхронизация плана с фактическим кодом,
   - выполнение следующего незавершённого пункта плана.
4. Обновите:
   - локальный план (checkpoint-блок),
   - этот реестр (`status`, `progress`, `last_updated_at`, `last_checkpoint`, `next_action`, `blockers`).
5. Сдвиньте `next_plan_id` на следующую запись по кругу (даже если текущий план заблокирован или завершён).

## Статусы

- `not_started` — работа не начата.
- `in_progress` — есть активная итерация.
- `blocked` — есть внешний блокер, требуется разблокировка.
- `done` — план завершён, verification пройден, docs синхронизированы.
- `archived` — план закрыт/заменён другим документом.

## Шаблон checkpoint-блока для локальных планов

В начало каждого implementation plan добавляйте и поддерживайте блок:

```md
## Execution checkpoint

- Current phase:
- Last checkpoint:
- Next step:
- Open blockers:
- Hand-off notes for next agent:
- Last updated at (UTC):
```

## Cycle state

| Field | Value | Notes |
|---|---|---|
| `cycle_id` | `2026-Q2-round-robin-v1` | Идентификатор текущего цикла |
| `next_plan_id` | `rustok-fulfillment` | ID записи, которую должен взять следующий агент |
| `last_rotation_at` | `2026-05-24T00:00:00Z` | Когда указатель был сдвинут последний раз |
| `rotation_rule` | `strict_round_robin` | Всегда следующий план по списку, без пропусков |

## Global board

| Plan ID | Module / crate | Plan doc | Status | Progress | Owner | Last updated (UTC) | Last checkpoint | Next action | Blockers | Verification gate |
|---|---|---|---|---|---|---|---|---|---|---|
| `alloy` | `alloy` | `crates/alloy/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p alloy --lib` |
| `flex` | `flex` | `crates/flex/docs/implementation-plan.md` | `in_progress` | `80%` | `agent` | `2026-05-24T21:44:13Z` | `plan-sync завершён: flex зафиксирован как capability-only ghost module; standalone GraphQL/REST уже live, открытым остаётся verification gate полного server integration run` | Закрыть Phase 4.5 verification debt: устранить compile-drift в `crates/rustok-product/src/services/catalog.rs`, затем прогнать `cargo test -p rustok-server --lib` + flex-targeted integration и зафиксировать evidence | blocker внешний к flex: compile-drift в `rustok-product` мешает полному `rustok-server` integration run | `cargo test -p flex --lib` |
| `leptos-auth` | `leptos-auth` | `crates/leptos-auth/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-auth --lib` |
| `leptos-graphql` | `leptos-graphql` | `crates/leptos-graphql/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-graphql --lib` |
| `leptos-hook-form` | `leptos-hook-form` | `crates/leptos-hook-form/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-hook-form --lib` |
| `leptos-shadcn-pagination` | `leptos-shadcn-pagination` | `crates/leptos-shadcn-pagination/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-shadcn-pagination --lib` |
| `leptos-table` | `leptos-table` | `crates/leptos-table/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-table --lib` |
| `leptos-zod` | `leptos-zod` | `crates/leptos-zod/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-zod --lib` |
| `leptos-zustand` | `leptos-zustand` | `crates/leptos-zustand/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p leptos-zustand --lib` |
| `rustok-ai` | `rustok-ai` | `crates/rustok-ai/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-ai --lib` |
| `rustok-ai-content` | `rustok-ai-content` | `crates/rustok-ai-content/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-ai-content --lib` |
| `rustok-ai-product` | `rustok-ai-product` | `crates/rustok-ai-product/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-ai-product --lib` |
| `rustok-ai-order` | `rustok-ai-order` | `crates/rustok-ai-order/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-ai-order --lib` |
| `rustok-api` | `rustok-api` | `crates/rustok-api/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-api --lib` |
| `rustok-auth` | `rustok-auth` | `crates/rustok-auth/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-auth --lib` |
| `rustok-blog` | `rustok-blog` | `crates/rustok-blog/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-blog --lib` |
| `rustok-cache` | `rustok-cache` | `crates/rustok-cache/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-cache --lib` |
| `rustok-cart` | `rustok-cart` | `crates/rustok-cart/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-cart --lib` |
| `rustok-channel` | `rustok-channel` | `crates/rustok-channel/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-channel --lib` |
| `rustok-comments` | `rustok-comments` | `crates/rustok-comments/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-comments --lib` |
| `rustok-commerce` | `rustok-commerce` | `crates/rustok-commerce/docs/implementation-plan.md` | `in_progress` | `46%` | `agent` | `2026-05-30T12:17:00Z` | `Phase 10.4 claim/exchange decision coupling: return decision tree now completes returns with refund/order_change resolution links for return_only/refund/exchange/claim and live REST/GraphQL claim parity` | Расширить post-order operator UX и связать exchange/claim return decisions с order-change apply/cancel lifecycle без host-owned logic | default server OpenAPI test блокируется существующими compile errors вне commerce; targeted order claim-resolution, commerce bridge tests, admin REST/GraphQL claim parity и commerce check проходят | `cargo check -p rustok-commerce`; `cargo test -p rustok-order complete_order_return_supports_claim_resolution_with_order_change -- --exact`; `cargo test -p rustok-commerce --test order_returns_bridge_test`; `cargo test -p rustok-commerce --test checkout_service_test complete_checkout_rejects_reentry_for_checking_out_cart_without_artifacts -- --exact`; `cargo test -p rustok-commerce admin_return_decision_transport_creates_claim_order_change -- --exact`; `cargo test -p rustok-commerce --test graphql_runtime_parity_test admin_graphql_return_decision_creates_completed_claim_order_change -- --exact` |
| `rustok-commerce-foundation` | `rustok-commerce-foundation` | `crates/rustok-commerce-foundation/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-commerce-foundation --lib` |
| `rustok-content` | `rustok-content` | `crates/rustok-content/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-content --lib` |
| `rustok-core` | `rustok-core` | `crates/rustok-core/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-core --lib` |
| `rustok-customer` | `rustok-customer` | `crates/rustok-customer/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-customer --lib` |
| `rustok-email` | `rustok-email` | `crates/rustok-email/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-email --lib` |
| `rustok-events` | `rustok-events` | `crates/rustok-events/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-events --lib` |
| `rustok-forum` | `rustok-forum` | `crates/rustok-forum/docs/implementation-plan.md` | `in_progress` | `82%` | `agent` | `2026-05-30T00:00:00Z` | `закрыт FW-1 contract freeze (design/contract path): machine-readable widget catalog/compatibility matrix/error mapping в manifest + REST/GraphQL contract surfaces + regression test на approved-only storefront replies` | Подготовить FW-2 design-prep: fallback checklist (`builder_off/publish_off`) и anti-5xx regression matrix без открытия tenant rollout delivery до P5 | pilot delivery по FW-2..FW-4 остаётся blocked до central P5 Wave 1 Go/No-Go + cross-runtime parity evidence + owner sign-off Platform/Builder/Forum/Frontend | `cargo test -p rustok-forum --lib` |
| `rustok-fulfillment` | `rustok-fulfillment` | `crates/rustok-fulfillment/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-fulfillment --lib` |
| `rustok-iggy` | `rustok-iggy` | `crates/rustok-iggy/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-iggy --lib` |
| `rustok-iggy-connector` | `rustok-iggy-connector` | `crates/rustok-iggy-connector/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-iggy-connector --lib` |
| `rustok-index` | `rustok-index` | `crates/rustok-index/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-index --lib` |
| `rustok-inventory` | `rustok-inventory` | `crates/rustok-inventory/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-inventory --lib` |
| `rustok-mcp` | `rustok-mcp` | `crates/rustok-mcp/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-mcp --lib` |
| `rustok-media` | `rustok-media` | `crates/rustok-media/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-media --lib` |
| `rustok-order` | `rustok-order` | `crates/rustok-order/docs/implementation-plan.md` | `in_progress` | `40%` | `agent` | `2026-05-28T00:00:00Z` | `Order returns lifecycle foundation: tenant-scoped get/list, complete/cancel transitions, transition guards и targeted tests` | Добавить item-level return lines и расширить docs/README под post-order guarantees | default server OpenAPI test блокируется существующими compile errors вне order; targeted lifecycle tests проходят | `cargo test -p rustok-order order_return_lifecycle --test order_service_test` |
| `rustok-outbox` | `rustok-outbox` | `crates/rustok-outbox/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-outbox --lib` |
| `rustok-pages` | `rustok-pages` | `crates/rustok-pages/docs/implementation-plan.md` | `in_progress` | `72%` | `agent` | `2026-05-25T00:00:00Z` | `PB-FBA-1A закрыт: machine-readable builder registry + anti-drift gate фиксируют provider/consumer versions, consumer_min_version и fallback profiles; scope остаётся PB-FBA-1B..1D` | Закрыть PB-FBA-1B typed errors parity для Next/Leptos/Flutter adapters; service fallback gate + admin/storefront host-helper static checks по `all_on/publish_off/preview_off/builder_off` уже покрыты | Wave 1 readiness остаётся `hold`, если есть waiver по anti-drift/fallback или нет полного evidence packet | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs` |
| `rustok-payment` | `rustok-payment` | `crates/rustok-payment/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-payment --lib` |
| `rustok-pricing` | `rustok-pricing` | `crates/rustok-pricing/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-pricing --lib` |
| `rustok-product` | `rustok-product` | `crates/rustok-product/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-product --lib` |
| `rustok-profiles` | `rustok-profiles` | `crates/rustok-profiles/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-profiles --lib` |
| `rustok-rbac` | `rustok-rbac` | `crates/rustok-rbac/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-rbac --lib` |
| `rustok-region` | `rustok-region` | `crates/rustok-region/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-region --lib` |
| `rustok-search` | `rustok-search` | `crates/rustok-search/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-search --lib` |
| `rustok-seo` | `rustok-seo` | `crates/rustok-seo/docs/implementation-plan.md` | `in_progress` | `92%` | `agent` | `2026-05-30T12:00:00Z` | `закрыт D2: publish path пишет seo_event_deliveries, связывает delivery с outbox envelope id и блокирует duplicate emission; продвинут D4: REST diagnostics/sitemaps/bulk jobs + GraphQL seoSitemapJobs/seoSitemapJob` | Закрыть D4 error-envelope unification и перейти к D3 SEO->index consumer seam (tenant/kind reindex + retry/DLQ policy) | для D3 нужен cross-owner sign-off `seo + outbox + index`; D4 error envelope требует единый contract decision между REST и GraphQL owners | `cargo check -p rustok-seo --tests --config profile.dev.debug=0` |
| `rustok-seo-render` | `rustok-seo-render` | `crates/rustok-seo/render/docs/implementation-plan.md` | `in_progress` | `35%` | `agent` | `2026-05-28T23:58:00Z` | `план синхронизирован с SEO Phase D: добавлены D7/D8 parity snapshots и Rust-vs-Next contract fixture backlog` | Закрыть D7.1: snapshot matrix для canonical/hreflang/robots/JSON-LD ordering | verification blocked в VM (`cargo` missing), плюс ожидание стабильного D4 REST/GraphQL parity contract | `cargo check -p rustok-seo-render --tests --config profile.dev.debug=0` |
| `rustok-seo-admin-support` | `rustok-seo-admin-support` | `crates/rustok-seo-admin-support/docs/implementation-plan.md` | `in_progress` | `55%` | `agent` | `2026-05-30T12:00:00Z` | `план синхронизирован с SEO Phase D после D2/D4 инкремента: REST/GraphQL transport parity endpoint baseline уже доступен, фокус смещён на D6.1 owner-side observability/remediation widgets` | Закрыть D6.1: reusable event delivery status cards + diagnostics remediation hints для owner-module panels | verification blocked в VM (`cargo` missing); для D6.1 требуется owner-side UI wiring в `pages/product/blog/forum` | `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0` |
| `rustok-storage` | `rustok-storage` | `crates/rustok-storage/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-storage --lib` |
| `rustok-tax` | `rustok-tax` | `crates/rustok-tax/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-tax --lib` |
| `rustok-taxonomy` | `rustok-taxonomy` | `crates/rustok-taxonomy/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-taxonomy --lib` |
| `rustok-telemetry` | `rustok-telemetry` | `crates/rustok-telemetry/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-telemetry --lib` |
| `rustok-tenant` | `rustok-tenant` | `crates/rustok-tenant/docs/implementation-plan.md` | `in_progress` | `90%` | `agent` | `2026-05-21T13:30:00Z` | `закрыт contract-sync между tenant module docs/manifest и server resolver contract; verification gates обновлены под фактическое tenant + resolver coverage` | Стартовать Iteration 2: lifecycle hardening (cache invalidation integration coverage для create/update/deactivate/domain-change) | `-` | `cargo test -p rustok-tenant --lib` |
| `rustok-test-utils` | `rustok-test-utils` | `crates/rustok-test-utils/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-test-utils --lib` |
| `rustok-workflow` | `rustok-workflow` | `crates/rustok-workflow/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-workflow --lib` |

## Round-robin protocol (для агентов)

1. Взять `next_plan_id` из `Cycle state`.
2. Выполнить один осмысленный инкремент по плану (sync + execution).
3. Обновить checkpoint в локальном плане.
4. Обновить статус в этом реестре.
5. Вычислить следующую запись по таблице `Global board` и записать её в `next_plan_id`.
6. Если возник блокер — перевести запись в `blocked` и явно зафиксировать условие разблокировки.

## Recovery protocol: второй агент без контекста

Если новый агент не знает, где остановился предыдущий:

1. Считать `next_plan_id` из `Cycle state` как единственный источник истины.
2. Открыть строку этого `Plan ID` в `Global board` и взять `Plan doc`.
3. В `Plan doc` прочитать только `Execution checkpoint` и `Quality backlog` (без полного перечитывания всего файла).
4. Если checkpoint пустой/устаревший — сделать мини-sync: обновить checkpoint, выставить `in_progress`, задать `next_action` и продолжить итерацию.
5. По завершении обязательно сдвинуть `next_plan_id` на следующую строку по кругу.

## Cross-module changes policy (минимально)

1. Если пункт плана требует правки в другом/дочернем модуле — это разрешено.
2. Делайте только нужный минимум для закрытия текущего пункта (без лишнего scope).
3. Для совместной фичи/правки достаточно коротко отметить затронутые модули в `Last checkpoint` или `Next action`.
4. Проверки запускайте для исходного и затронутого модулей.

## Bugfix / Refactor policy при актуализации планов

Во время итерации по плану агент **может и должен** исправлять найденные ошибки и делать рефакторинг,
но только в контролируемом объёме:

1. Если проблема напрямую блокирует текущий пункт плана — исправлять в этой же итерации.
2. Если изменение небольшое и локальное (в пределах текущего модуля/контракта) — допускается включать в тот же инкремент.
3. Если проблема крупная или cross-cutting — не расширять scope молча: добавить отдельный пункт в backlog,
   зафиксировать в `blockers`/`next_action` и пройти по round-robin дальше.
4. Любой bugfix/refactor, отмеченный как `done`, должен пройти соответствующий verification gate.
5. После исправления обязательно синхронизировать локальный `implementation-plan.md` и checkpoint.

## Definition of done для пунктов плана

Пункт плана можно пометить `done` только если одновременно:

1. Изменение присутствует в коде.
2. Пройден соответствующий verification gate.
3. Локальный `implementation-plan.md` обновлён под фактическое состояние.

## Registry sync при изменении числа модулей

Синхронизацию состава `Global board` делаем по событию завершения полного круга (а не по календарю):

1. Триггер: `end_of_full_cycle` (вернулись к стартовому `Plan ID`).
2. Сверить `Global board` со списком `crates/*/docs/implementation-plan.md`.
3. Добавить missing строки для новых модулей/библиотек.
4. Удалить orphaned строки для удалённых модулей/библиотек.
5. Для rename/relocate обновить существующую строку (`Plan ID`, `Plan doc`, `Verification gate`) без создания дубля.

## Weekly sweep

Раз в неделю отдельный агент/ответственный выполняет sweep:

- отмечает stale-элементы (`last_updated_at` старше 7 дней),
- поднимает приоритеты для `blocked` записей,
- формирует краткий список "next up" для нового круга.

## Hygiene: как чистить таблицу, если раздулась

Чтобы реестр оставался рабочим и не разрастался бесполезной историей:

1. Держите в `Global board` только live-записи (`not_started`, `in_progress`, `blocked`, `done` за последние 14 дней).
2. Старые завершённые записи удаляйте из реестра (без отдельного архивного файла).
3. Сохраняйте только действительно важный контекст: в `implementation-plan.md` (раздел critical context) или в `DECISIONS/` для архитектурных решений.
4. Если у плана сменился путь/название — обновляйте текущую строку, а не создавайте дубль.
5. При каждом weekly sweep удаляйте пустые/дублированные строки и проверяйте уникальность `Plan ID`.
