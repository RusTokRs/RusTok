# Архитектура GraphQL и Leptos server functions

- **Дата:** 2026-04-02
- **Статус:** Актуальный transport contract для UI-контуров
- **Связанные ADR:** `DECISIONS/2026-03-29-leptos-server-functions-as-internal-data-layer.md`, `DECISIONS/2026-03-07-deployment-profiles-and-ui-stack.md`

## Главное правило

Для Leptos UI в RusToK действует **dual-path модель**:

- native `#[server]` functions — основной внутренний data-layer для `apps/admin`, `apps/storefront` и module-owned Leptos UI packages;
- GraphQL (`/api/graphql`) — обязательный параллельный контракт, который **не удаляется** и остаётся доступен для headless-клиентов, Next.js UI и fallback-веток в Leptos.

`#[server]` не заменяет GraphQL на уровне платформы. Он добавляет более короткий внутренний путь для Leptos.

## Матрица по UI-стекам

| Контур | Основной transport | Параллельный/обязательный transport |
|---|---|---|
| `apps/admin` | `#[server]` | GraphQL |
| `apps/storefront` | `#[server]` | GraphQL |
| module-owned Leptos UI crates | `#[server]` | GraphQL |
| `apps/next-admin` | GraphQL | — |
| `apps/next-frontend` | GraphQL | — |
| mobile / external integrations | GraphQL | — |

## Базовый паттерн для Leptos

```text
UI component
  -> local api function
  -> try native #[server]
  -> on unsupported/error path fallback to GraphQL
  -> service layer / DB
```

### В монолите

```text
HTTP request -> Axum -> Leptos SSR -> #[server] fn -> service layer -> DB
```

Во время SSR это может быть in-process вызов без GraphQL resolver-слоя.

### В hydration / client navigation / standalone Leptos

```text
browser -> POST /api/fn/* -> server -> service layer -> DB
```

Это всё ещё native Leptos transport, но уже через HTTP endpoint `leptos_axum`.

### GraphQL path

```text
client -> POST /api/graphql -> GraphQL resolver -> service layer -> DB
```

GraphQL остаётся:

- публичным backend contract;
- транспортом для Next.js UI;
- fallback-веткой для Leptos, пока native coverage не полная;
- transport surface для persisted queries и совместимости со старыми модулями.

## Что обязаны делать новые Leptos модули

### Если модуль поставляет Leptos UI

- Не ходить из UI напрямую в `reqwest`/GraphQL как в единственный путь.
- Добавлять локальный API-слой рядом с UI crate или host integration.
- Для read/write сценариев сначала проектировать native `#[server]` boundary.
- Сохранять GraphQL query/mutation path как совместимый параллельный transport.

### Если модуль поставляет только Next.js UI

- Использовать GraphQL как основной transport.
- Не требовать от Next.js клиента `#[server]` contracts.

## Что обязаны делать host-приложения

### `apps/admin`

- Поддерживать `csr`, `ssr`, `hydrate` профили там, где это уже введено.
- Для Leptos data access использовать native-first pattern.
- Не удалять существующие GraphQL queries/mutations без отдельного решения уровня платформы.

### `apps/storefront`

- Для host shell и module-owned storefront packages использовать native-first pattern.
- Оставлять GraphQL path доступным для fallback и headless parity.

### `apps/server`

- Регистрировать `/api/fn/*` через `leptos_axum::handle_server_fns()`.
- Сохранять `/api/graphql` как отдельный живой route.
- Не трактовать внедрение server functions как повод убирать GraphQL schema/resolvers.

## Что запрещено

- Формулировать документацию как “GraphQL-only для Leptos”, если код уже использует `#[server]`.
- Формулировать документацию как “Leptos больше не использует GraphQL вообще”, если GraphQL сохранён как fallback/parallel contract.
- Создавать новый module-owned Leptos UI пакет с единственным transport path через GraphQL, когда native `#[server]` слой возможен.
- Удалять GraphQL route/resolvers при миграции Leptos UI на native path.

## Проверка документации и кода

При изменении Leptos UI нужно синхронизировать:

- `AGENTS.md`
- `docs/AI_CONTEXT.md`
- `docs/UI/storefront.md`
- локальные `README.md` и `docs/README.md` в `apps/admin`, `apps/storefront`, `apps/server`
- README module-owned Leptos UI crates, если у них есть собственный data-layer

Контрольный вопрос для нового UI-кода:

> Если это Leptos UI, есть ли рядом native `#[server]` path и сохранился ли GraphQL как параллельный контракт?
