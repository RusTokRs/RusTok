# Маршрутизация и границы transport-слоя

Этот документ фиксирует границы между GraphQL, REST, module-owned HTTP surfaces
и internal Leptos `#[server]` functions.

## Основное правило

В RusToK transport layer делится по назначению, а не по вкусу команды:

- GraphQL — основной UI-facing contract
- REST — интеграции, webhooks, ops и совместимые transport flows
- `#[server]` functions — internal data layer для Leptos hosts и module-owned UI
- health/metrics endpoints — operational surface

Новый endpoint должен вписываться в один из этих каналов, а не создавать
четвёртый локальный transport style.

## Матрица выбора

| Сценарий | Канал |
|---|---|
| Admin/storefront UI query/mutation | GraphQL |
| Leptos internal UI action | `#[server]` function |
| Внешняя интеграция | REST |
| Webhook ingress / callback | REST |
| Health / readiness / metrics | Operational endpoints |
| OpenAPI discovery | REST/OpenAPI |

## GraphQL

GraphQL используется как единый UI-facing contract:

- `apps/admin`
- `apps/storefront`
- `apps/next-admin`
- `apps/next-frontend`
- module-owned UI packages, если им нужен GraphQL transport

GraphQL не должен размываться в integration-only flows, где нужен стабильный
REST contract.

## `#[server]`-функции

Для Leptos hosts и module-owned Leptos UI `#[server]` functions являются
предпочтительным internal data layer.

При этом:

- GraphQL не удаляется и остаётся параллельным contract
- `#[server]` functions не должны становиться заменой внешнего API
- ownership бизнес-логики остаётся у module/service layer, а не у UI crate

## REST

REST используется для:

- внешних интеграций
- webhook callback flows
- служебных операций
- совместимых transport surfaces, где GraphQL не подходит
- module-owned HTTP endpoints, если модулю нужен именно HTTP contract

REST не должен дублировать UI-facing GraphQL без явной причины.

## Module-owned маршрутизация

Если модуль публикует HTTP routes или UI surfaces:

- routing объявляется через `rustok-module.toml`
- host application только монтирует surface
- source of truth для wiring живёт в manifest и local docs модуля

Наличие controller или UI sub-crate без manifest wiring не считается полным
contract.

## Route-selection contract для module-owned admin UI

Для module-owned admin UI действует единый platform contract:

- selection state хранится в query string и считается URL-owned source of truth;
- используются только typed `snake_case` query keys вроде `product_id`, `cart_id`, `order_id`, `thread_id`,
  `media_id`, `channel_id`, `topic_id`, `provider_slug`, `tool_profile_slug`, `task_profile_slug`,
  а для SEO route — `target_kind`, `target_id`, `locale`, `tab`;
- generic `id`, camelCase keys и прочие legacy aliases не читаются и не canonicalize’ятся;
- отсутствие валидного selection key означает empty state, а не auto-select-first;
- invalid nested keys очищаются локально и не должны ломать соседние selection domains;
- смена subpath/tab обязана prune’ить keys, невалидные для destination page.

Разделение ownership:

- `rustok-api` владеет typed query schema, invariant rules и sanitization contract;
- `leptos-ui-routing` остаётся generic Leptos route/query helper без admin-specific key registry;
- host-приложения (`apps/admin`, `apps/next-admin`) владеют route writers/adapters и обязаны
  держать parity по одному и тому же query contract.

## Query contract для module-owned storefront UI

Для module-owned storefront UI действует тот же ownership split, но без admin-specific typed
selection schema:

- host/runtime передаёт `UiRouteContext` с effective locale, route base и canonical query snapshot;
- storefront packages читают свои domain query keys через общий helper layer, а не через
  package-local route parsing;
- для Leptos storefront packages query reads идут через `leptos-ui-routing`, а не через
  direct `UiRouteContext.query_value(...)`;
- `leptos-ui-routing` остаётся generic helper и не владеет storefront key registry,
  canonical slugs, locale policy или module-specific invariants;
- `apps/storefront` и `apps/next-frontend` обязаны держать parity по query semantics,
  locale propagation и canonical route behavior, не создавая второй query policy поверх
  backend/host contract.

## Locale и маршрутизация

Locale-routing определяется host/runtime layer:

- Leptos и Next hosts используют host-provided effective locale
- module-owned UI packages не вводят собственную query/header/cookie chain
- locale contract должен совпадать с `docs/UI/*` и локальными docs приложений

## Что не делать

- не использовать GraphQL как транспорт для внешнего webhook callback
- не выносить integration-only REST contract в `#[server]` functions
- не дублировать один и тот же UI flow в GraphQL и REST без причины
- не прятать module-owned routing только в host application

## Связанные документы

- [Архитектура API](./api.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Быстрый старт для Admin ↔ Server](../UI/admin-server-connection-quickstart.md)
- [Архитектура модулей](./modules.md)
