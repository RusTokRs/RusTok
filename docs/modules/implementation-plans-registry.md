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
| `next_plan_id` | `rustok-iggy-connector` | ID записи, которую должен взять следующий агент |
| `last_rotation_at` | `2026-06-14T00:00:00Z` | Когда указатель был сдвинут последний раз |
| `rotation_rule` | `strict_round_robin` | Всегда следующий план по списку, без пропусков |

## Global board

| Plan ID | Module / crate | Plan doc | Status | Progress | Owner | Last updated (UTC) | Last checkpoint | Next action | Blockers | Verification gate |
|---|---|---|---|---|---|---|---|---|---|---|
| `alloy` | `alloy` | `crates/alloy/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p alloy --lib` |
| `flex` | `flex` | `crates/flex/docs/implementation-plan.md` | `in_progress` | `82%` | `agent` | `2026-06-13T00:00:00Z` | `no-compile инкремент: product-side metadata update path patched to preserve reserved metadata during Flex custom-field updates; helper tests added` | При разрешённых компиляциях прогнать `cargo test -p rustok-product --lib`, затем `cargo test -p rustok-server --lib` + flex-targeted integration и зафиксировать evidence | compile/test evidence отложен по явному ограничению итерации: без компиляций | `cargo test -p flex --lib` |
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
| `rustok-comments` | `rustok-comments` | `crates/rustok-comments/docs/implementation-plan.md` | `in_progress` | `38%` | `agent` | `2026-06-07T00:00:00Z` | `Comments admin core теперь владеет transport request/command DTO construction; transport facade/native adapter принимают core DTO вместо UI-built primitive argument lists` | Добавить GraphQL/headless fallback adapter поверх тех же core DTO или contract-freeze evidence для native-only comments admin wave | GraphQL fallback для comments admin пока не реализован; FBA остается not_started до boundary evidence | `cargo check -p rustok-comments-admin --config profile.dev.debug=0`; `cargo test -p rustok-comments-admin --lib --config profile.dev.debug=0`; `npm run verify:ffa:ui:migration` |
| `rustok-commerce` | `rustok-commerce` | `crates/rustok-commerce/docs/implementation-plan.md` | `in_progress` | `52%` | `agent` | `2026-06-13T04:36:00Z` | `Scaffolded and registered the next-admin @rustok/commerce-admin package, exposing GraphQL client wrappers for shipping profiles, cart promotions, and order changes to enable Next.js transport parity.` | Добавить страницы/компоненты администрирования в @rustok/commerce-admin для вызова зарегистрированных GraphQL-адаптеров | default server OpenAPI test блокируется существующими compile errors вне commerce; fast inventory boundary verifier and selected commerce/inventory source guardrails pass | `node scripts/verify/verify-inventory-admin-boundary.mjs`; `./scripts/verify/verify-all.sh inventory-admin-boundary`; `cargo check -p rustok-commerce`; targeted storefront/cart/checkout inventory availability integration tests после добавления coverage |
| `rustok-commerce-foundation` | `rustok-commerce-foundation` | `crates/rustok-commerce-foundation/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-commerce-foundation --lib` |
| `rustok-content` | `rustok-content` | `crates/rustok-content/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-content --lib` |
| `rustok-core` | `rustok-core` | `crates/rustok-core/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-core --lib` |
| `rustok-customer` | `rustok-customer` | `crates/rustok-customer/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-customer --lib` |
| `rustok-email` | `rustok-email` | `crates/rustok-email/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-email --lib` |
| `rustok-events` | `rustok-events` | `crates/rustok-events/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-events --lib` |
| `rustok-forum` | `rustok-forum` | `crates/rustok-forum/docs/implementation-plan.md` | `in_progress` | `82%` | `agent` | `2026-05-30T00:00:00Z` | `закрыт FW-1 contract freeze (design/contract path): machine-readable widget catalog/compatibility matrix/error mapping в manifest + REST/GraphQL contract surfaces + regression test на approved-only storefront replies` | Подготовить FW-2 design-prep: fallback checklist (`builder_off/publish_off`) и anti-5xx regression matrix без открытия tenant rollout delivery до P5 | pilot delivery по FW-2..FW-4 остаётся blocked до central P5 Wave 1 Go/No-Go + cross-runtime parity evidence + owner sign-off Platform/Builder/Forum/Frontend | `cargo test -p rustok-forum --lib` |
| `rustok-fulfillment` | `rustok-fulfillment` | `crates/rustok-fulfillment/docs/implementation-plan.md` | `in_progress` | `18%` | `agent` | `2026-06-13T22:35:00Z` | `Fulfillment storefront now owns seller-aware shipping selection DTOs, core request normalization and Leptos selection panel; commerce checkout route renders that owner UI and retains only transitional aggregate selection transport callback.` | Move select-shipping-option transport facade/server-function from commerce compatibility into `rustok-fulfillment/storefront` with GraphQL fallback parity, then update commerce docs/registry evidence. | `-` | `node scripts/verify/verify-fulfillment-admin-boundary.mjs`; `node scripts/verify/verify-fulfillment-storefront-boundary.mjs`; `cargo test -p rustok-fulfillment --lib` |
| `rustok-iggy` | `rustok-iggy` | `crates/rustok-iggy/docs/implementation-plan.md` | `in_progress` | `18%` | `agent` | `2026-06-14T00:00:00Z` | `no-compile: добавлен consume_next_as_group поверх connector subscribe, serializer deserialize contract и fake-connector tests; connector is_connected исправлен как cross-module prerequisite` | Довести offset/ack metadata и связать DLQ/replay movement с consume path | `compile/test evidence отложен по явному ограничению итерации: без компиляций` | `cargo test -p rustok-iggy --lib` |
| `rustok-iggy-connector` | `rustok-iggy-connector` | `crates/rustok-iggy-connector/docs/implementation-plan.md` | `in_progress` | `10%` | `agent` | `2026-06-14T00:00:00Z` | `no-compile cross-module prerequisite: remote/embedded is_connected() now reads actual connection state` | Формализовать subscriber metadata contract для offset/ack/retry semantics без transport policy leakage | `compile/test evidence отложен по явному ограничению итерации: без компиляций` | `cargo test -p rustok-iggy-connector --lib` |
| `rustok-index` | `rustok-index` | `crates/rustok-index/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-index --lib` |
| `rustok-inventory` | `rustok-inventory` | `crates/rustok-inventory/docs/implementation-plan.md` | `in_progress` | `62%` | `agent` | `2026-06-07T08:10:00Z` | `Wave 5 inventory admin boundary current scope complete: native-only AdminInventoryReadService/server-function read path, native set/adjust/reserve/release/check-availability facade, removed commerce GraphQL fallback, public-channel availability/projection helpers exported for commerce compatibility` | Перейти к verification/CI evidence и поддерживать новые admin операции только через module-owned facade; non-admin/channel-aware availability хвост ведётся в rustok-commerce roadmap | Первый CI migration-smoke прогон ещё нужно наблюдать; channel-aware availability integration coverage ведётся как commerce compatibility work | `node scripts/verify/verify-inventory-admin-boundary.mjs`; `./scripts/verify/verify-all.sh inventory-admin-boundary`; `node scripts/verify/verify-inventory-admin-boundary.test.mjs`; `cargo test -p rustok-inventory --lib` |
| `rustok-mcp` | `rustok-mcp` | `crates/rustok-mcp/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-mcp --lib` |
| `rustok-media` | `rustok-media` | `crates/rustok-media/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-media --lib` |
| `rustok-order` | `rustok-order` | `crates/rustok-order/docs/implementation-plan.md` | `in_progress` | `40%` | `agent` | `2026-05-28T00:00:00Z` | `Order returns lifecycle foundation: tenant-scoped get/list, complete/cancel transitions, transition guards и targeted tests` | Добавить item-level return lines и расширить docs/README под post-order guarantees | default server OpenAPI test блокируется существующими compile errors вне order; targeted lifecycle tests проходят | `cargo test -p rustok-order order_return_lifecycle --test order_service_test` |
| `rustok-outbox` | `rustok-outbox` | `crates/rustok-outbox/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-outbox --lib` |
| `rustok-pages` | `rustok-pages` | `crates/rustok-pages/docs/implementation-plan.md` | `in_progress` | `74%` | `agent` | `2026-06-01T00:00:00Z` | `PB-FBA-1B частично закрыт: degraded_modes связаны с typed runtime error catalog через provider/consumer metadata, FBA registry, runtime constants и anti-drift gate; Next Admin, Leptos и Flutter app-core typed-error parity добавлены в baseline gate; Wave evidence template и синтетический Wave 0 dry-run packet добавлены как machine-readable contracts; fallback profiles остаются зелёными` | Провести реальный PB-FBA-1C control-plane dry-run и заменить синтетический packet фактическими before/after snapshots по evidence template | Wave 1 readiness остаётся `hold`, если есть waiver по anti-drift/fallback или нет полного фактического evidence packet | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs` |
| `rustok-payment` | `rustok-payment` | `crates/rustok-payment/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-payment --lib` |
| `rustok-pricing` | `rustok-pricing` | `crates/rustok-pricing/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-pricing --lib` |
| `rustok-product` | `rustok-product` | `crates/rustok-product/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-product --lib` |
| `rustok-profiles` | `rustok-profiles` | `crates/rustok-profiles/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-profiles --lib` |
| `rustok-rbac` | `rustok-rbac` | `crates/rustok-rbac/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-rbac --lib` |
| `rustok-region` | `rustok-region` | `crates/rustok-region/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-region --lib` |
| `rustok-search` | `rustok-search` | `crates/rustok-search/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Синхронизировать план с текущим кодом и заполнить checkpoint | `-` | `cargo test -p rustok-search --lib` |
| `rustok-seo` | `rustok-seo` | `crates/rustok-seo/docs/implementation-plan.md` | `in_progress` | `96%` | `agent` | `2026-06-08T10:20:00Z` | `D7..D9 перегруппированы в Milestones A..E; закрыты A.1-A.3 и B.1-B.3 (Next runtime adapter REST-first+GraphQL fallback, runtime robots/sitemap, home metadata consume + JSON-LD runtime rendering)` | Закрыть A.4/B.4 как единый пакет: fallback fixtures/evidence + runtime metadata smoke минимум на двух non-home owner routes | route ownership matrix beyond home route и explicit long-tail diff allowlist пока отсутствуют; без них нельзя закрыть Milestone C/D verification gates | `cargo check -p rustok-seo --tests --config profile.dev.debug=0` |
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
