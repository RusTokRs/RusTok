# Документация по модулям RusToK

Этот документ фиксирует актуальную модульную модель RusToK без смешения архитектурных терминов и технической упаковки.

## Базовая модель

Для platform modules существует только два статуса:

- `Core`
- `Optional`

Источник истины по ним — `modules.toml`.

При этом:

- `crate` — это термин Cargo и способ упаковки;
- не каждый crate в `crates/` является platform module;
- рядом с module-crates лежат shared libraries и infrastructure/support crates.

## Где смотреть в коде

- состав platform modules: `modules.toml`
- runtime registry: `apps/server/src/modules/mod.rs`
- сверка manifest и registry: `apps/server/src/modules/manifest.rs`
- базовые модульные контракты: `crates/rustok-core/src/module.rs`
- категории `Core` / `Optional`: `crates/rustok-core/src/registry.rs`

## Текущий состав platform modules

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
| `product` | `rustok-product` | — |
| `profiles` | `rustok-profiles` | — |
| `pricing` | `rustok-pricing` | `product` |
| `inventory` | `rustok-inventory` | `product` |
| `order` | `rustok-order` | — |
| `payment` | `rustok-payment` | — |
| `fulfillment` | `rustok-fulfillment` | — |
| `commerce` | `rustok-commerce` | `cart`, `customer`, `product`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment` |
| `blog` | `rustok-blog` | `content` |
| `forum` | `rustok-forum` | `content` |
| `pages` | `rustok-pages` | `content` |
| `media` | `rustok-media` | — |
| `workflow` | `rustok-workflow` | — |

## Важное уточнение по wiring

`ModuleRegistry` — это runtime composition point, а не классификатор архитектурных ролей.

Из этого следуют два правила:

1. Если компонент объявлен как platform module в `modules.toml`, он обязан быть либо `Core`, либо `Optional`.
2. Технический способ подключения модуля может отличаться:
   - регистрация в `ModuleRegistry`
   - bootstrap/runtime wiring
   - generated host wiring

`rustok-outbox` — хороший пример. Он является `Core` module и одновременно используется напрямую event runtime слоем. Это не делает его отдельным “третьим типом”.

## Что лежит рядом с модулями

В `crates/` также живут компоненты, которые не входят в taxonomy `Core/Optional`:

- shared libraries: `rustok-core`, `rustok-api`, `rustok-events`, `rustok-storage`, `rustok-test-utils`
- infra/capability crates: `rustok-iggy`, `rustok-iggy-connector`, `rustok-telemetry`, `rustok-mcp`, `alloy`, `alloy-scripting`, `flex`

Именно поэтому нельзя автоматически приравнивать “любой crate в `crates/`” к platform module.

## UI composition policy

Если у модуля есть UI, он должен поставляться самим модулем:

- Leptos: через sub-crates `admin/` и `storefront/`
- Next.js: через пакеты в `apps/next-admin/packages/*` и `apps/next-frontend/packages/*`

Host-приложения должны монтировать эти поверхности через manifest-driven wiring, а не через жёстко пришитые module-specific ветки.

## Связанные документы

- [Реестр модулей и приложений](./registry.md)
- [Реестр crate-ов RusToK](./crates-registry.md)
- [Манифест модулей](./manifest.md)
- [Архитектура модулей](../architecture/modules.md)
