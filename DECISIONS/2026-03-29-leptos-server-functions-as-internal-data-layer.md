# Leptos `#[server]` functions как внутренний слой данных для Leptos-приложений

- Date: 2026-03-29
- Status: Accepted, amended on 2026-04-02
- Supersedes: `2026-03-07-deployment-profiles-and-ui-stack.md` (в части транспорта между Leptos UI и сервером)

## Context

Leptos-контур RusToK теперь поддерживает два transport path одновременно:

- GraphQL HTTP (`/api/graphql`);
- Leptos server functions (`/api/fn/*`).

Изначальная формулировка ADR была слишком жёсткой и трактовала `#[server]`
как полную замену GraphQL для Leptos UI. По факту код и platform rule пошли в
другую сторону: native path добавляется **параллельно**, а GraphQL остаётся
живым transport contract.

Это нужно по трём причинам:

1. GraphQL уже является внешним контрактом для Next.js, мобильных клиентов и интеграций.
2. Миграция Leptos host-приложений и module-owned UI crates происходит поэтапно, coverage не везде полная.
3. Даже после ввода native path платформа не хочет терять GraphQL как совместимый transport и fallback.

## Decision

### Принцип

**Leptos `#[server]`-функции становятся основным внутренним data-layer для
Leptos UI, но не заменяют GraphQL на уровне платформы.**

То есть:

- для `apps/admin`, `apps/storefront` и module-owned Leptos UI packages путь по умолчанию:

```text
UI -> local api -> #[server] -> service layer -> DB
```

- GraphQL остаётся обязательным параллельным transport:

```text
client -> /api/graphql -> GraphQL resolver -> service layer -> DB
```

### Что это означает для Leptos UI

#### Монолит / SSR

```text
HTTP request -> Axum -> Leptos SSR -> #[server] fn -> service layer -> DB
```

Во многих SSR-сценариях это in-process путь без GraphQL resolver-слоя.

#### Hydration / client navigation / standalone Leptos

```text
browser -> POST /api/fn/* -> server -> service layer -> DB
```

Это нативный Leptos transport через `leptos_axum`.

#### Параллельный GraphQL path

GraphQL остаётся:

- внешним API для `apps/next-admin`, `apps/next-frontend`, mobile и integrations;
- fallback-веткой для Leptos UI там, где native coverage ещё не полная;
- transport surface для старых модулей и persisted-query сценариев.

### Правило для новых модулей

Если новый модуль поставляет Leptos UI:

- нельзя проектировать его как GraphQL-only data path, если возможен native `#[server]` слой;
- нужно добавлять local API boundary и native-first вызовы;
- GraphQL query/mutation path нельзя удалять, если он уже существует или нужен для внешних клиентов.

## Deployment consequences

### Монолит

```text
browser -> Axum -> Leptos SSR -> #[server] fn -> service layer -> DB
```

Native path минимизирует внутренний transport overhead, но `/api/graphql`
остаётся поднятым и доступным.

### Headless

```text
Leptos -> POST /api/fn/*
Next.js / external clients -> POST /api/graphql
```

Оба transport surface живут рядом.

## Consequences

### Позитивные

- Monolith получает короткий native data path для Leptos UI.
- GraphQL не теряется как публичный контракт.
- Миграция host-приложений и module-owned UI crates возможна по частям.
- Один и тот же модуль может обслуживать и Leptos UI, и внешние headless-клиенты.

### Негативные

- Нужно поддерживать dual-path документацию и verification.
- Leptos-коду нужен явный local API layer, а не прямые вызовы transport из view.
- Нельзя бездумно удалять старые GraphQL operations, даже если рядом уже есть `#[server]`.

## Follow-up

1. Документация должна везде фиксировать dual-path rule: native `#[server]` first, GraphQL parallel.
2. `apps/server` обязан держать и `/api/graphql`, и `/api/fn/*`.
3. Новые module-owned Leptos UI crates должны следовать той же схеме.
4. Verification-планы должны проверять не только GraphQL, но и Leptos server functions transport.
