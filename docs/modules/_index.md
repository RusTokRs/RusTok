# Индекс документации по модулям

Локальная документация модулей живёт внутри самих crate-ов в
`crates/<name>/docs/README.md`. Этот документ даёт только центральную навигацию по
локальным docs модулей и support/capability crate-ов и не дублирует локальные контракты.

## Правило навигации

- модульная документация не дублируется в `docs/modules/`;
- ссылки ниже ведут прямо в `crates/<name>/docs/`;
- для платформенных модулей обязательный минимум: `README.md`, `docs/README.md`,
  `docs/implementation-plan.md`.

## Контракт документации

- root `README.md` у компонента остаётся публичным контрактом на английском и описывает
  `Purpose`, `Responsibilities`, `Entry points` и `Interactions`;
- локальный `docs/README.md` на русском остаётся живым runtime/module-контрактом;
- локальный `docs/implementation-plan.md` на русском остаётся живым планом доведения
  компонента до целевого состояния;
- этот индекс нужен только для навигации и должен отправлять читателя в локальные docs,
  а не пересказывать их содержание.

## Core и foundation-слой

| Компонент | Документация | План реализации |
|---|---|---|
| `rustok-core` | [docs](../../crates/rustok-core/docs/README.md) | [plan](../../crates/rustok-core/docs/implementation-plan.md) |
| `rustok-events` | [docs](../../crates/rustok-events/docs/README.md) | [plan](../../crates/rustok-events/docs/implementation-plan.md) |
| `rustok-channel` | [docs](../../crates/rustok-channel/docs/README.md) | [plan](../../crates/rustok-channel/docs/implementation-plan.md) |
| `rustok-index` | [docs](../../crates/rustok-index/docs/README.md) | [plan](../../crates/rustok-index/docs/implementation-plan.md) |
| `rustok-search` | [docs](../../crates/rustok-search/docs/README.md) | [plan](../../crates/rustok-search/docs/implementation-plan.md) |
| `rustok-outbox` | [docs](../../crates/rustok-outbox/docs/README.md) | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) |
| `rustok-telemetry` | [docs](../../crates/rustok-telemetry/docs/README.md) | [plan](../../crates/rustok-telemetry/docs/implementation-plan.md) |
| `rustok-tenant` | [docs](../../crates/rustok-tenant/docs/README.md) | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) |
| `rustok-rbac` | [docs](../../crates/rustok-rbac/docs/README.md) | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) |
| `rustok-cache` | [docs](../../crates/rustok-cache/docs/README.md) | [plan](../../crates/rustok-cache/docs/implementation-plan.md) |
| `rustok-auth` | [docs](../../crates/rustok-auth/docs/README.md) | [plan](../../crates/rustok-auth/docs/implementation-plan.md) |
| `rustok-email` | [docs](../../crates/rustok-email/docs/README.md) | [plan](../../crates/rustok-email/docs/implementation-plan.md) |
| `rustok-storage` | [docs](../../crates/rustok-storage/docs/README.md) | [plan](../../crates/rustok-storage/docs/implementation-plan.md) |
| `rustok-api` | [docs](../../crates/rustok-api/docs/README.md) | [plan](../../crates/rustok-api/docs/implementation-plan.md) |
| `rustok-test-utils` | [docs](../../crates/rustok-test-utils/docs/README.md) | [plan](../../crates/rustok-test-utils/docs/implementation-plan.md) |
| `rustok-iggy` | [docs](../../crates/rustok-iggy/docs/README.md) | [plan](../../crates/rustok-iggy/docs/implementation-plan.md) |
| `rustok-iggy-connector` | [docs](../../crates/rustok-iggy-connector/docs/README.md) | [plan](../../crates/rustok-iggy-connector/docs/implementation-plan.md) |
| `rustok-mcp` | [docs](../../crates/rustok-mcp/docs/README.md) | [plan](../../crates/rustok-mcp/docs/implementation-plan.md) |
| `rustok-ai` | [docs](../../crates/rustok-ai/docs/README.md) | [plan](../../crates/rustok-ai/docs/implementation-plan.md) |
| `alloy` | [docs](../../crates/alloy/docs/README.md) | [plan](../../crates/alloy/docs/implementation-plan.md) |
| `flex` | [docs](../../crates/flex/docs/README.md) | [plan](../../crates/flex/docs/implementation-plan.md) |
| `rustok-commerce-foundation` | [docs](../../crates/rustok-commerce-foundation/docs/README.md) | [plan](../../crates/rustok-commerce-foundation/docs/implementation-plan.md) |

## Доменные модули

| Компонент | Документация | План реализации |
|---|---|---|
| `rustok-content` | [docs](../../crates/rustok-content/docs/README.md) | [plan](../../crates/rustok-content/docs/implementation-plan.md) |
| `rustok-cart` | [docs](../../crates/rustok-cart/docs/README.md) | [plan](../../crates/rustok-cart/docs/implementation-plan.md) |
| `rustok-customer` | [docs](../../crates/rustok-customer/docs/README.md) | [plan](../../crates/rustok-customer/docs/implementation-plan.md) |
| `rustok-product` | [docs](../../crates/rustok-product/docs/README.md) | [plan](../../crates/rustok-product/docs/implementation-plan.md) |
| `rustok-profiles` | [docs](../../crates/rustok-profiles/docs/README.md) | [plan](../../crates/rustok-profiles/docs/implementation-plan.md) |
| `rustok-region` | [docs](../../crates/rustok-region/docs/README.md) | [plan](../../crates/rustok-region/docs/implementation-plan.md) |
| `rustok-pricing` | [docs](../../crates/rustok-pricing/docs/README.md) | [plan](../../crates/rustok-pricing/docs/implementation-plan.md) |
| `rustok-tax` | [docs](../../crates/rustok-tax/docs/README.md) | [plan](../../crates/rustok-tax/docs/implementation-plan.md) |
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

## UI-пакеты модулей

### Core/admin-поверхности

- `rustok-channel` admin UI: [README](../../crates/rustok-channel/admin/README.md)
- `rustok-index` admin UI: [README](../../crates/rustok-index/admin/README.md)
- `rustok-outbox` admin UI: [README](../../crates/rustok-outbox/admin/README.md)
- `rustok-tenant` admin UI: [README](../../crates/rustok-tenant/admin/README.md)
- `rustok-rbac` admin UI: [README](../../crates/rustok-rbac/admin/README.md)

### Optional/admin-поверхности

- `rustok-product` admin UI: [README](../../crates/rustok-product/admin/README.md)
- `rustok-fulfillment` admin UI: [README](../../crates/rustok-fulfillment/admin/README.md)
- `rustok-customer` admin UI: [README](../../crates/rustok-customer/admin/README.md)
- `rustok-region` admin UI: [README](../../crates/rustok-region/admin/README.md)
- `rustok-order` admin UI: [README](../../crates/rustok-order/admin/README.md)
- `rustok-inventory` admin UI: [README](../../crates/rustok-inventory/admin/README.md)
- `rustok-pricing` admin UI: [README](../../crates/rustok-pricing/admin/README.md)
- `rustok-commerce` admin UI: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-pages` admin UI: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum` admin UI: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search` admin UI: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media` admin UI: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments` admin UI: [README](../../crates/rustok-comments/admin/README.md)

### Optional/storefront-поверхности

- `rustok-blog` storefront UI: [README](../../crates/rustok-blog/storefront/README.md)
- `rustok-cart` storefront UI: [README](../../crates/rustok-cart/storefront/README.md)
- `rustok-commerce` storefront UI: [README](../../crates/rustok-commerce/storefront/README.md)
- `rustok-forum` storefront UI: [README](../../crates/rustok-forum/storefront/README.md)
- `rustok-pages` storefront UI: [README](../../crates/rustok-pages/storefront/README.md)
- `rustok-pricing` storefront UI: [README](../../crates/rustok-pricing/storefront/README.md)
- `rustok-product` storefront UI: [README](../../crates/rustok-product/storefront/README.md)
- `rustok-region` storefront UI: [README](../../crates/rustok-region/storefront/README.md)
- `rustok-search` storefront UI: [README](../../crates/rustok-search/storefront/README.md)

### Capability/admin-поверхности

- `rustok-ai` Leptos operator/admin UI: [README](../../crates/rustok-ai/admin/README.md)
- `rustok-ai` Next.js operator/admin UI: `apps/next-admin/packages/rustok-ai/`

## Примечания

- `rustok-content` остаётся shared helper/orchestration boundary и не публикует
  отдельный operator-facing UI.
- UI split ecommerce family уже начат: `rustok-product` публикует собственный
  admin package, `rustok-fulfillment` уже забрал shipping-option UI, `rustok-order`
  уже забрал order UI, `rustok-inventory` уже забрал inventory visibility UI,
  `rustok-pricing` уже забрал pricing visibility UI, `rustok-customer` уже
  забрал customer operations UI, `rustok-region` уже забрал region CRUD UI, а
  `rustok-commerce-admin` оставлен под shipping-profile registry и aggregate cart-promotion operator surface;
  storefront-side split тоже продвинут: `rustok-region`, `rustok-product`, `rustok-pricing` и `rustok-cart` уже публикуют собственные
  storefront packages, а `rustok-commerce-storefront` сжат до aggregate checkout workspace с seller-aware delivery-group shipping selection и без catalog/pricing ownership;
  остальные commerce storefront flows ещё предстоит вынести из umbrella route там, где ownership boundary уже устойчива.
- `rustok-mcp` и `rustok-ai` считаются capability/support
  layers и индексируются здесь для навигации, даже если не входят в taxonomy
  `Core/Optional`; при этом `rustok-ai` уже публикует крупные operator/admin
  UI-поверхности для Leptos и Next.js host-ов.
- `flex` тоже остаётся capability-layer по своей роли, но теперь формализован в
  `modules.toml` как `capability_only` ghost module; donor persistence ownership
  при этом всё равно остаётся у модулей-потребителей.
- при изменении runtime-контракта или ownership сначала обновляются локальные docs
  компонента, затем этот индекс и остальные central registry docs.

## Связанные документы

- [Обзор модульной платформы](./overview.md)
- [Реестр модулей и приложений](./registry.md)
- [Реестр crate-ов модульной платформы](./crates-registry.md)
- [Контракт `rustok-module.toml`](./manifest.md)
- [Шаблон документации модуля](../templates/module_contract.md)
### Next.js admin showcase

- `rustok-blog`: `apps/next-admin/packages/blog/`
- `rustok-search`: `apps/next-admin/packages/search/`

## Примечание по `module-system`

- Финальный repo-side статус `Registry V1/V2`, authenticated governance contract, `registry_only` и thin runner path фиксируется в соответствующих ADR внутри `DECISIONS/` и в живых platform docs.
- Audit UI-классификации path-модулей (`dual-surface` / `admin-only` / `storefront-only` / `no-ui`) тоже ведётся там; этот индекс остаётся только навигацией по локальным docs.
