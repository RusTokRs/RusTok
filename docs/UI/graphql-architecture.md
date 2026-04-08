# GraphQL и Leptos server functions

Этот документ фиксирует текущий transport contract для UI-контуров RusToK.

## Основное правило

Для Leptos UI в платформе действует dual-path модель:

- native `#[server]` functions — основной внутренний data-layer для `apps/admin`, `apps/storefront` и module-owned Leptos UI packages;
- GraphQL `/api/graphql` — обязательный параллельный transport contract для Next.js hosts, headless clients и fallback-веток в Leptos.

`#[server]` не заменяет GraphQL на уровне платформы. Он добавляет более короткий внутренний путь для Leptos hosts.

## Матрица по UI hosts

| Host | Основной transport | Обязательный параллельный transport |
|------|--------------------|--------------------------------------|
| `apps/admin` | `#[server]` | GraphQL |
| `apps/storefront` | `#[server]` | GraphQL |
| module-owned Leptos UI | `#[server]` | GraphQL |
| `apps/next-admin` | GraphQL | — |
| `apps/next-frontend` | GraphQL | — |
| external/mobile clients | GraphQL | — |

## Contract для Leptos UI

- Leptos host или module-owned package должен сначала проектировать локальный API-слой поверх `#[server]`.
- Если native path ещё не покрывает нужный сценарий, допускается fallback к GraphQL.
- Новый Leptos UI не должен проектироваться как GraphQL-only, если `#[server]` path реалистичен.
- GraphQL queries и mutations нельзя убирать только потому, что появился native путь.

Базовый паттерн:

```text
UI component
  -> local API function
  -> try native #[server]
  -> fallback to GraphQL when required
  -> service layer
```

## Contract для GraphQL

GraphQL остаётся:

- публичным backend contract;
- основным transport-слоем для Next.js hosts;
- fallback-путём для Leptos hosts;
- transport surface для websocket subscriptions и совместимости с headless clients.

Security и allow/deny policy для чувствительных admin-операций должны определяться server-side runtime-слоем, а не client-supplied `operationName` или app-local эвристиками.

## Обязанности host-приложений

### `apps/admin`

- использовать native-first pattern для Leptos data access;
- сохранять GraphQL path как живой parallel contract;
- не переносить transport policy в app-local ad hoc код.

### `apps/storefront`

- использовать native-first pattern для host shell и module-owned storefront packages;
- сохранять GraphQL path для fallback и parity с headless storefront clients.

### `apps/server`

- держать `/api/fn/*` и `/api/graphql` как параллельные runtime surfaces;
- не трактовать внедрение server functions как повод убирать GraphQL schema или resolvers;
- применять shared policy к HTTP GraphQL и websocket execution path одинаково.

## Что запрещено

- описывать Leptos UI как GraphQL-only, если в коде уже существует `#[server]` path;
- описывать Leptos migration как отказ от GraphQL вообще;
- удалять GraphQL route или resolver только из-за появления native Leptos transport;
- вводить разные transport contracts для app host и module-owned UI без явного platform-level решения.

## Связанные документы

- [UI index](./README.md)
- [Storefront contract](./storefront.md)
- [Документация `apps/admin`](../../apps/admin/docs/README.md)
- [Документация `apps/storefront`](../../apps/storefront/docs/README.md)
- [Документация `apps/server`](../../apps/server/docs/README.md)
- [Карта документации](../index.md)
