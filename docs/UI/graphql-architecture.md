# GraphQL и Leptos server functions

Этот документ фиксирует текущий transport contract для UI-контуров RusToK.

## Основное правило

Для Leptos UI в платформе действует dual-path модель поверх SSR-first runtime:

- native `#[server]` functions — preferred internal data-layer для `apps/admin`, `apps/storefront` и module-owned Leptos UI packages в `ssr`/`hydrate`/monolith профилях;
- GraphQL `/api/graphql` — обязательный параллельный transport contract для Next.js hosts, headless clients и fallback-веток в Leptos.

`#[server]` не заменяет GraphQL на уровне платформы. Он добавляет более короткий внутренний путь для Leptos hosts, когда host реально работает через SSR/hydrate runtime.

CSR остаётся обязательным compatibility/debug профилем для standalone Trunk/WASM и проверки module-owned UI packages, но не считается продуктовым runtime default.

## Почему так

- SSR/hydrate лучше соответствует monolith deployment: `apps/admin` и `apps/storefront` работают same-origin с `apps/server`, используют server-side auth/session/policy и не требуют лишней CORS/proxy схемы.
- `#[server]` даёт самый короткий внутренний Rust-путь от Leptos UI к service layer и не заставляет превращать каждое внутреннее действие host-а в публичную GraphQL mutation.
- GraphQL/REST остаются обязательными, потому что headless — отдельный продуктовый режим для Next.js hosts, внешних клиентов, интеграций и мобильных приложений.
- CSR/Trunk оставлен как debug/compatibility профиль: он нужен для локальной проверки module-owned UI packages и ловит случайные server-only зависимости в WASM-сборке.
- Решение применяется не только к `apps/admin` и `apps/storefront`, а ко всем module-owned UI packages в `crates/*/admin` и `crates/*/storefront`, потому что host только монтирует эти поверхности.

## Матрица по UI hosts

| Host / profile | Runtime default | Preferred transport | Обязательный параллельный transport |
|------|-----------------|--------------------|--------------------------------------|
| `apps/admin` `ssr`/`hydrate` | SSR-first monolith | `#[server]` | GraphQL/REST |
| `apps/storefront` `ssr`/`hydrate` | SSR-first monolith | `#[server]` | GraphQL/REST |
| module-owned Leptos UI in SSR hosts | SSR/hydrate | `#[server]` | GraphQL/REST |
| module-owned Leptos UI standalone `csr` | debug/compatibility | GraphQL/REST | — |
| `apps/next-admin` | headless | GraphQL/REST | — |
| `apps/next-frontend` | headless | GraphQL/REST | — |
| external/mobile clients | headless | GraphQL/REST | — |

## Contract для Leptos UI

- Leptos host или module-owned package должен сначала проектировать локальный API-слой под SSR/hydrate `#[server]` path, если surface является внутренним Leptos runtime surface.
- Если native path ещё не покрывает нужный сценарий или surface должна работать в standalone `csr`, требуется fallback к GraphQL/REST.
- Новый Leptos UI не должен проектироваться как GraphQL-only для monolith runtime, если `#[server]` path реалистичен.
- Новый Leptos UI не должен проектироваться как `#[server]`-only, если surface нужна для standalone CSR debug или headless parity.
- GraphQL queries и mutations нельзя убирать только потому, что появился native путь.

Базовый паттерн:

```text
UI component
  -> local API function
  -> in SSR/hydrate: try native #[server]
  -> in CSR/headless-compatible path: use GraphQL/REST fallback
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

- считать SSR/hydrate preferred production runtime для monolith;
- использовать native-first pattern для Leptos data access в SSR/hydrate;
- сохранять GraphQL path как живой parallel contract;
- поддерживать CSR compatibility для standalone debug через GraphQL/REST, без обязательного `/api/fn/*`;
- не переносить transport policy в app-local ad hoc код.

### `apps/storefront`

- считать SSR/hydrate preferred production runtime для monolith;
- использовать native-first pattern для host shell и module-owned storefront packages в SSR/hydrate;
- сохранять GraphQL path для fallback и parity с headless storefront clients.

### `apps/server`

- держать `/api/fn/*` и `/api/graphql` как параллельные runtime surfaces;
- не трактовать внедрение server functions как повод убирать GraphQL schema или resolvers;
- применять shared policy к HTTP GraphQL и websocket execution path одинаково.

## Что запрещено

- описывать Leptos UI как GraphQL-only, если в коде уже существует `#[server]` path;
- описывать Leptos migration как отказ от GraphQL вообще;
- описывать CSR/Trunk как production default для Leptos hosts;
- удалять GraphQL route или resolver только из-за появления native Leptos transport;
- вводить разные transport contracts для app host и module-owned UI без явного platform-level решения.

## Связанные документы

- [UI index](./README.md)
- [Storefront contract](./storefront.md)
- [Документация `apps/admin`](../../apps/admin/docs/README.md)
- [Документация `apps/storefront`](../../apps/storefront/docs/README.md)
- [Документация `apps/server`](../../apps/server/docs/README.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Карта документации](../index.md)
