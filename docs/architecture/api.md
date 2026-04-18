# Архитектура API

Политика выбора API-стиля описана в [routing.md](./routing.md). Этот документ
фиксирует верхнеуровневую карту API surfaces RusToK.

## Краткое резюме

RusToK использует гибридный transport layer:

- GraphQL для UI-клиентов
- REST для интеграций, webhooks, ops и module-owned HTTP contracts
- `#[server]` functions для internal Leptos data layer
- OpenAPI для машиночитаемого REST contract
- health/metrics endpoints для observability

## Канонические эндпоинты

| Surface | Endpoint | Назначение |
|---|---|---|
| GraphQL | `/api/graphql` | Единая точка для admin/storefront UI |
| GraphQL WS | `/api/graphql/ws` | Subscriptions transport |
| REST | `/api/v1/...` | Интеграции, webhooks, batch/ops scenarios |
| Commerce REST | `/store/...`, `/admin/...` | Совместимые ecommerce HTTP flows |
| OpenAPI | `/api/openapi.json`, `/api/openapi.yaml` | REST contract discovery |
| Health | `/health`, `/health/live`, `/health/ready` | Health and readiness |
| Metrics | `/metrics` | Observability and scraping |

## Владение API-поверхностью

- `apps/server` владеет общим API host layer
- platform modules владеют domain contracts, resolvers, handlers и service layer
- host applications и UI packages не должны становиться canonical owner API logic
- module-owned HTTP/GraphQL-поверхности должны совпадать с manifest wiring и local docs

## GraphQL-поверхность

GraphQL остаётся canonical UI-facing contract для:

- Leptos hosts
- Next.js hosts
- module-owned UI packages

GraphQL должен собирать domain data через module/service layer, а не обходить
ownership модулей через host-specific shortcuts.

## REST-поверхность

REST остаётся обязательным для сценариев, где нужен явный HTTP contract:

- внешние интеграции
- webhooks
- operational endpoints
- совместимые ecommerce flows
- module-owned transport routes
- для post-order ecommerce surface первый OMS slice уже включает admin refund routes поверх `payment-collections` (`/admin/payment-collections/{id}/refunds`, `/admin/refunds/{id}/complete`, `/admin/refunds/{id}/cancel`)

REST не должен использоваться как скрытая замена GraphQL для UI-only flows.

## `#[server]`-поверхность

Leptos `#[server]` functions — это internal host/UI contract, а не замена
публичного API surface.

Правила:

- `#[server]` functions по умолчанию используются внутри Leptos hosts и
  module-owned Leptos UI
- GraphQL сохраняется параллельно
- external integrations не завязываются на `#[server]`

## Безопасность и контракт контекста

Каждый API path должен работать через единый host/runtime context:

- tenant resolution
- request-scoped `ChannelContext`, включая `resolution_source` и `resolution_trace` для channel-aware runtime diagnostics
- auth/session handling
- RBAC enforcement
- request-scoped locale
- observability hooks

API surface не должен обходить эти слои через локальные shortcuts.

## Что не делать

- не смешивать ownership API contract между host и module crate
- не дублировать transport flows без явной причины
- не считать UI package источником правды для API surface
- не вводить отдельный locale/auth contract на уровне конкретного endpoint family

## Связанные документы

- [Маршрутизация и границы transport-слоя](./routing.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Архитектура модулей](./modules.md)
- [Обзор архитектуры платформы](./overview.md)
