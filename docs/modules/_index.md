# Индекс документации по модулям

Локальная документация каждого модуля живёт внутри соответствующего crate. Эта страница — единая точка навигации по scoped-документам.

## Правило размещения

- документация модуля хранится в `crates/<name>/docs/`;
- root `README.md` остаётся обязательным английским описанием crate и должен содержать `## Purpose`, `## Responsibilities`, `## Entry points`, `## Interactions` и ссылку на `docs/README.md`;
- `docs/README.md` и `docs/implementation-plan.md` обязательны для каждого path-модуля из `modules.toml`.

## Core и platform foundation

| Модуль | Документация | План |
|---|---|---|
| `rustok-auth` | [docs](../../crates/rustok-auth/docs/README.md) | [plan](../../crates/rustok-auth/docs/implementation-plan.md) |
| `rustok-cache` | [docs](../../crates/rustok-cache/docs/README.md) | [plan](../../crates/rustok-cache/docs/implementation-plan.md) |
| `rustok-channel` | [docs](../../crates/rustok-channel/docs/README.md) | [plan](../../crates/rustok-channel/docs/implementation-plan.md) |
| `rustok-email` | [docs](../../crates/rustok-email/docs/README.md) | [plan](../../crates/rustok-email/docs/implementation-plan.md) |
| `rustok-index` | [docs](../../crates/rustok-index/docs/README.md) | [plan](../../crates/rustok-index/docs/implementation-plan.md) |
| `rustok-outbox` | [docs](../../crates/rustok-outbox/docs/README.md) | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) |
| `rustok-rbac` | [docs](../../crates/rustok-rbac/docs/README.md) | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) |
| `rustok-search` | [docs](../../crates/rustok-search/docs/README.md) | [plan](../../crates/rustok-search/docs/implementation-plan.md) |
| `rustok-tenant` | [docs](../../crates/rustok-tenant/docs/README.md) | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) |

## Domain modules

| Модуль | Документация | План |
|---|---|---|
| `rustok-content` | [docs](../../crates/rustok-content/docs/README.md) | [plan](../../crates/rustok-content/docs/implementation-plan.md) |
| `rustok-cart` | [docs](../../crates/rustok-cart/docs/README.md) | [plan](../../crates/rustok-cart/docs/implementation-plan.md) |
| `rustok-customer` | [docs](../../crates/rustok-customer/docs/README.md) | [plan](../../crates/rustok-customer/docs/implementation-plan.md) |
| `rustok-product` | [docs](../../crates/rustok-product/docs/README.md) | [plan](../../crates/rustok-product/docs/implementation-plan.md) |
| `rustok-profiles` | [docs](../../crates/rustok-profiles/docs/README.md) | [plan](../../crates/rustok-profiles/docs/implementation-plan.md) |
| `rustok-region` | [docs](../../crates/rustok-region/docs/README.md) | [plan](../../crates/rustok-region/docs/implementation-plan.md) |
| `rustok-pricing` | [docs](../../crates/rustok-pricing/docs/README.md) | [plan](../../crates/rustok-pricing/docs/implementation-plan.md) |
| `rustok-inventory` | [docs](../../crates/rustok-inventory/docs/README.md) | [plan](../../crates/rustok-inventory/docs/implementation-plan.md) |
| `rustok-order` | [docs](../../crates/rustok-order/docs/README.md) | [plan](../../crates/rustok-order/docs/implementation-plan.md) |
| `rustok-payment` | [docs](../../crates/rustok-payment/docs/README.md) | [plan](../../crates/rustok-payment/docs/implementation-plan.md) |
| `rustok-fulfillment` | [docs](../../crates/rustok-fulfillment/docs/README.md) | [plan](../../crates/rustok-fulfillment/docs/implementation-plan.md) |
| `rustok-commerce` | [docs](../../crates/rustok-commerce/docs/README.md) | [plan](../../crates/rustok-commerce/docs/implementation-plan.md) |
| `rustok-blog` | [docs](../../crates/rustok-blog/docs/README.md) | [plan](../../crates/rustok-blog/docs/implementation-plan.md) |
| `rustok-comments` | [docs](../../crates/rustok-comments/docs/README.md) | [plan](../../crates/rustok-comments/docs/implementation-plan.md) |
| `rustok-forum` | [docs](../../crates/rustok-forum/docs/README.md) | [plan](../../crates/rustok-forum/docs/implementation-plan.md) |
| `rustok-pages` | [docs](../../crates/rustok-pages/docs/README.md) | [plan](../../crates/rustok-pages/docs/implementation-plan.md) |
| `rustok-taxonomy` | [docs](../../crates/rustok-taxonomy/docs/README.md) | [plan](../../crates/rustok-taxonomy/docs/implementation-plan.md) |
| `rustok-media` | [docs](../../crates/rustok-media/docs/README.md) | [plan](../../crates/rustok-media/docs/implementation-plan.md) |
| `rustok-workflow` | [docs](../../crates/rustok-workflow/docs/README.md) | [plan](../../crates/rustok-workflow/docs/implementation-plan.md) |

## Capability crates в scoped-аудите

| Модуль | Документация | План |
|---|---|---|
| `alloy` | [docs](../../crates/alloy/docs/README.md) | [plan](../../crates/alloy/docs/implementation-plan.md) |

## UI-пакеты

- `rustok-channel` admin UI: [README](../../crates/rustok-channel/admin/README.md)
- `rustok-index` admin UI: [README](../../crates/rustok-index/admin/README.md)
- `rustok-outbox` admin UI: [README](../../crates/rustok-outbox/admin/README.md)
- `rustok-tenant` admin UI: [README](../../crates/rustok-tenant/admin/README.md)
- `rustok-rbac` admin UI: [README](../../crates/rustok-rbac/admin/README.md)
- `rustok-commerce` admin UI: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-pages` admin UI: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum` admin UI: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search` admin UI: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media` admin UI: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments` admin UI: [README](../../crates/rustok-comments/admin/README.md)

## Связанные документы

- [Обзор модульной платформы](./overview.md)
- [Контракт `rustok-module.toml`](./manifest.md)
- [Карта модулей и владельцев](./registry.md)
- [Исследование по единому стандарту модулей](../research/deep-research-modules.md)
