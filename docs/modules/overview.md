# Обзор модульной платформы

Этот документ фиксирует актуальную модульную модель RusToK без смешения
архитектурных ролей и технической упаковки.

Central docs в `docs/modules/` описывают карту платформы, taxonomy и правила
композиции, но не заменяют локальные docs самих компонентов.

## Базовая модель

Для платформенных модулей существует только две категории:

- `Core`
- `Optional`

Источник истины по составу и зависимостям платформенных модулей: `modules.toml`.

При этом важно различать:

- платформенный модуль — crate, который объявлен в `modules.toml` и входит в runtime-taxonomy `Core` или `Optional`;
- support/library crate — shared dependency или инфраструктурный слой, который живёт в `crates/`, но не является платформенным модулем;
- capability crate — отдельный runtime capability-layer, который может подключаться к платформе, но не обязан входить в `Core/Optional` taxonomy.

## Источники истины по документации

- корневой `README.md` компонента на английском фиксирует публичный контракт:
  `Purpose`, `Responsibilities`, `Entry points`, `Interactions`;
- локальный `docs/README.md` на русском фиксирует живой runtime/module/app-контракт;
- локальный `docs/implementation-plan.md` на русском фиксирует живой план развития;
- central docs в `docs/modules/` связывают эту картину вместе и не должны
  дублировать локальные документы построчно.

## Где смотреть в коде

- состав платформенных модулей: `modules.toml`
- runtime registry: `apps/server/src/modules/mod.rs`
- manifest-wiring: `apps/server/src/modules/manifest.rs`
- базовые модульные контракты: `crates/rustok-core/src/module.rs`
- taxonomy `Core` / `Optional`: `crates/rustok-core/src/registry.rs`

## Platform modules

### Core

| Slug | Crate |
|---|---|
| `auth` | `rustok-auth` |
| `cache` | `rustok-cache` |
| `channel` | `rustok-channel` |
| `email` | `rustok-email` |
| `index` | `rustok-index` |
| `search` | `rustok-search` |
| `outbox` | `rustok-outbox` |
| `tenant` | `rustok-tenant` |
| `rbac` | `rustok-rbac` |

### Optional

| Slug | Crate | Depends on |
|---|---|---|
| `content` | `rustok-content` | — |
| `cart` | `rustok-cart` | — |
| `customer` | `rustok-customer` | — |
| `product` | `rustok-product` | `taxonomy` |
| `profiles` | `rustok-profiles` | `taxonomy` |
| `region` | `rustok-region` | — |
| `pricing` | `rustok-pricing` | `product` |
| `inventory` | `rustok-inventory` | `product` |
| `order` | `rustok-order` | — |
| `payment` | `rustok-payment` | — |
| `fulfillment` | `rustok-fulfillment` | — |
| `commerce` | `rustok-commerce` | `cart`, `customer`, `product`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment` |
| `blog` | `rustok-blog` | `content`, `comments`, `taxonomy` |
| `forum` | `rustok-forum` | `content`, `taxonomy` |
| `comments` | `rustok-comments` | — |
| `pages` | `rustok-pages` | `content` |
| `taxonomy` | `rustok-taxonomy` | `content` |
| `media` | `rustok-media` | — |
| `workflow` | `rustok-workflow` | — |

## Что лежит рядом с модулями

Не каждый crate в `crates/` является платформенным модулем.

### Shared libraries

- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`
- `rustok-commerce-foundation`

### Infrastructure / capability crates

- `rustok-iggy`
- `rustok-iggy-connector`
- `rustok-telemetry`
- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`

Именно поэтому нельзя автоматически отождествлять «любой crate в `crates/`» с
платформенный модуль.

При изменении ownership, runtime-контракта или boundaries компонента сначала
обновляются локальные docs этого компонента, затем `overview.md`, `registry.md`,
`_index.md` и остальные central registry docs.

## UI composition policy

Если модуль поставляет UI, этот UI должен оставаться module-owned:

- Leptos UI-поверхности публикуются через sub-crates `admin/` и `storefront/`;
- Next.js UI-поверхности публикуются через пакеты в `apps/next-admin/packages/*` и
  `apps/next-frontend/packages/*`;
- host-приложения монтируют эти UI-поверхности через manifest-driven wiring, а не
  через жёстко пришитые module-specific branches.

## Alloy и capability crates

`alloy`, `rustok-ai`, `rustok-mcp` и `flex` не входят в taxonomy `Core/Optional`
как обычные платформенные модули.

Это означает:

- они могут быть частью runtime-composition;
- они могут иметь собственные docs, UI и capability-поверхность;
- но их роль описывается как support/capability-слой, а не как tenant-toggled
  module category.

## Связанные документы

- [Реестр модулей и приложений](./registry.md)
- [Индекс документации по модулям](./_index.md)
- [Реестр crate-ов модульной платформы](./crates-registry.md)
- [Контракт `rustok-module.toml`](./manifest.md)
- [Спец-план rich-text и визуального page builder](./tiptap-page-builder-implementation-plan.md)
- [Архитектура модулей](../architecture/modules.md)
