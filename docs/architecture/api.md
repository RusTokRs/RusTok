---
id: doc://docs/architecture/api.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
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
| MCP management/runtime | `/api/mcp/...` | Persisted MCP clients/tokens/policies/audit и remote runtime bootstrap |
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
- mobile/headless hosts, включая Flutter admin/frontend clients

GraphQL должен собирать domain data через module/service layer, а не обходить
ownership модулей через host-specific shortcuts. Auth bootstrap для headless/mobile hosts использует `me.permissions` как UI-facing RBAC snapshot; server-side enforcement остаётся обязательным для самих mutations/queries.

Public storefront/read GraphQL queries не должны превращать отсутствие
`AuthContext` в `SecurityContext::system()`. Anonymous read flow использует
`SecurityContext::public_read()` (`SecurityActorKind::Public`) и обязан сохранять
module-level published/channel-visible filters рядом с read. `SecurityContext::system()`
разрешён только trusted platform runtime paths: bootstrap, jobs, migrations,
batch/internal providers и `PortActorKind::System`.

## REST-поверхность

REST остаётся обязательным для сценариев, где нужен явный HTTP contract:

- внешние интеграции
- webhooks
- operational endpoints
- совместимые ecommerce flows
- module-owned transport routes
- для post-order ecommerce surface первый OMS slice уже включает admin refund routes поверх `payment-collections` (`/admin/payment-collections/{id}/refunds`, `/admin/refunds/{id}/complete`, `/admin/refunds/{id}/cancel`)

REST не должен использоваться как скрытая замена GraphQL для UI-only flows.

MCP runtime bootstrap является platform-owned REST surface: `POST /api/mcp/runtime/bootstrap` принимает MCP Bearer token или `plaintext_token`, требует non-stdio transport, возвращает persisted runtime binding/effective access context и пишет audit event с correlation id.

## `#[server]`-поверхность

Leptos `#[server]` functions — это internal host/UI contract, а не замена
публичного API surface.

Правила:

- `#[server]` functions по умолчанию используются внутри Leptos hosts и
  module-owned Leptos UI
- GraphQL сохраняется параллельно
- external integrations не завязываются на `#[server]`

## Нейтральные портовые primitives

Для перевода модулей на Fluid Backend Architecture новый портовый слой должен
использовать shared primitives из `rustok-api::ports`:

- `PortContext` передаёт tenant, actor/service identity, claims/roles, channel, locale,
  correlation/causation, trace context, idempotency key и deadline semantics;
- `PortError` и `PortErrorKind` задают transport-agnostic доменную error envelope до mapping
  в GraphQL/REST/gRPC;
- `PortCallPolicy` и `PortOperationKind` задают reusable enforcement для read/write/event-replay/best-effort операций без module-specific behavior;
- read-порты должны проверять deadline semantics; write и event-replay порты должны проверять idempotency key и deadline до обращения к owner storage
  или remote adapter.

Эти types не являются application service layer и не должны содержать module-specific
business logic.

`rustok-api` владеет `Port*`, permission и locale primitives и не зависит от
`rustok-core` ни в одном feature. Runtime RBAC/security policy принадлежит core,
который зависит от API contract layer. Runtime-specific adapters также не входят в neutral contract surface:
outbox Loco wiring принадлежит `rustok-outbox::loco` и включается feature
`rustok-outbox/loco-adapter`.

## Безопасность и контракт контекста

Каждый API path должен работать через единый host/runtime context:

- tenant resolution
- request-scoped locale
- auth/session handling
- request-scoped `ChannelContext`, включая `resolution_source` и `resolution_trace` для channel-aware runtime diagnostics
- RBAC enforcement
- observability hooks

Для full application router канонический порядок подготовки request context:
`security_headers -> tenant::resolve -> locale::resolve_locale -> auth_context::resolve_optional -> channel::resolve -> handler`.
`channel::resolve` должен строить `RequestFacts` из tenant id, request selectors, effective host,
auth-derived OAuth/client dimension и effective locale; channel cache key обязан различать locale/OAuth dimensions,
чтобы request одного client/locale не переиспользовал resolution другого контекста.

API surface не должен обходить эти слои через локальные shortcuts.

### Проверка request-context/channel invariant

Для изменений в middleware, channel resolution, locale/auth extensions или cache key
обязателен быстрый source-level gate:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
./scripts/verify/verify-all.sh runtime-context-invariants
```

Эта проверка закрепляет следующие runtime contracts без полной Rust-компиляции:

- `build_request_facts` читает OAuth/client dimension из `AuthContextExtension`;
- `build_request_facts` читает effective locale из `ResolvedRequestLocale`;
- `ChannelCacheKey` содержит `oauth_app_id` и `locale`, включая negative cache entries;
- source-order middleware в `compose_application_router` сохраняет фактический Axum execution order
  `locale -> auth_context -> channel`;
- tenant locale cache metrics остаются экспортируемыми через `/metrics`.

Если проверка падает, нельзя чинить только текстовый порядок `.layer(...)`: нужно
восстановить фактическое поведение request extensions, cache-key dimensions и
observability evidence.


## Reference artifacts (DOC-09)

Для contract-level изменений API обязательны обновляемые reference-артефакты:

- OpenAPI snapshots (`/api/openapi.json`, `/api/openapi.yaml`)
- GraphQL introspection snapshot (`/api/graphql`)
- rustdoc artifacts для `rustok-server` и `rustok-workflow`

Канонический локальный экспорт выполняется через:

```bash
scripts/verify/export-reference-artifacts.sh artifacts/reference
```

Правило: при изменении GraphQL/REST/`#[server]` contract в PR должен быть
Verification Evidence по экспорту артефактов и ссылке на diff.

## Совместимость API

- GraphQL, REST, OpenAPI и `#[server]` contracts считаются публичными для своих
  целевых клиентов и не удаляются без documented migration path.
- Breaking change требует явного описания миграции в PR и обновления локальных
  module/app docs.
- Новый Leptos `#[server]` path не заменяет существующий GraphQL/REST contract,
  если этот contract уже используется как fallback или headless surface.
- Для revision-aware control-plane mutations stale client state должен получать
  conflict-style ошибку, а не silent overwrite или blind rollback.

## Tenant isolation и RLS

- Базовая модель остаётся shared DB/shared schema с `tenant_id` как обязательным
  application/runtime boundary.
- DB-level RLS является целевым hardening layer для high-risk tenant-scoped
  таблиц, но включается staged: сначала platform-control/tenant-module pilot
  после появления request-scoped tenant DB session context.
- Broad RLS big-bang миграция запрещена без отдельного ADR и rollback plan.

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
