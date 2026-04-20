# Документация по UI-пакетам модулей

Этот документ даёт навигацию по module-owned UI-поверхностям и фиксирует только
актуальный контрактный слой. Он не заменяет локальные docs самих модулей и не
дублирует их runtime/UI details.

## Базовое правило

- UI пакеты принадлежат самому модулю, а не host-приложению;
- Leptos admin/storefront UI-поверхности публикуются через sub-crates `admin/` и
  `storefront/` внутри module crate;
- Next.js host-приложения только монтируют module-owned UI-поверхности и не должны
  становиться их каноническим владельцем;
- источник истины для UI-wiring живёт в `rustok-module.toml`, локальном
  `README.md` и `docs/README.md` самого модуля.

## Что считать UI-пакетом

Для платформенного модуля UI-поверхность считается корректно оформленной, если есть:

- root `README.md` модуля на английском;
- локальный `docs/README.md` на русском;
- локальный `docs/implementation-plan.md` на русском;
- `rustok-module.toml` с корректным `[provides.admin_ui]` и/или
  `[provides.storefront_ui]`, если модуль реально поставляет UI;
- `admin/Cargo.toml` и/или `storefront/Cargo.toml`, если такой UI объявлен в
  manifest-wiring.

Само наличие папки `admin/` или `storefront/` не считается доказательством
интеграции. Канонический источник истины здесь только manifest-wiring.

## Runtime-контракт для UI-пакетов

- Leptos module-owned UI использует host-provided locale-контракт и не
  придумывает собственную цепочку locale fallback;
- для internal Leptos data layer по умолчанию используются `#[server]`
  functions, при этом GraphQL остаётся параллельным transport-контрактом;
- Next.js hosts работают через server/API-контракты и не дублируют module-owned
  domain logic в приложении;
- host-приложения отвечают только за mount/wiring/navigation, а не за
  ownership UI-функциональности модуля.

## Куда смотреть

### Общий контракт

- [Контракт `rustok-module.toml`](./manifest.md)
- [Реестр модулей и приложений](./registry.md)
- [Индекс документации по модулям](./_index.md)
- [Шаблон документации модуля](../templates/module_contract.md)

### UI и хост-приложения

- [Обзор UI](../UI/README.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Контракт storefront](../UI/storefront.md)
- [Быстрый старт для Admin ↔ Server](../UI/admin-server-connection-quickstart.md)

### Локальные docs приложений

- [Документация Admin](../../apps/admin/docs/README.md)
- [Документация Storefront](../../apps/storefront/docs/README.md)
- [Документация Next Admin](../../apps/next-admin/docs/README.md)
- [Документация Next Frontend](../../apps/next-frontend/docs/README.md)

## Примеры модульного UI

### Core/admin-поверхности

- `rustok-channel` admin UI: [README](../../crates/rustok-channel/admin/README.md)
- `rustok-index` admin UI: [README](../../crates/rustok-index/admin/README.md)
- `rustok-outbox` admin UI: [README](../../crates/rustok-outbox/admin/README.md)
- `rustok-tenant` admin UI: [README](../../crates/rustok-tenant/admin/README.md)
- `rustok-rbac` admin UI: [README](../../crates/rustok-rbac/admin/README.md)

### Optional/admin-поверхности

- `rustok-product` admin UI: [README](../../crates/rustok-product/admin/README.md)
- Ecommerce admin UI routes `rustok-product` <-> `rustok-pricing` поддерживают
  stable deep links через product `id`; display-поля не используются как identity.
- `rustok-fulfillment` admin UI: [README](../../crates/rustok-fulfillment/admin/README.md)
- `rustok-customer` admin UI: [README](../../crates/rustok-customer/admin/README.md)
- `rustok-region` admin UI: [README](../../crates/rustok-region/admin/README.md)
- `rustok-order` admin UI: [README](../../crates/rustok-order/admin/README.md)
- `rustok-inventory` admin UI: [README](../../crates/rustok-inventory/admin/README.md)
- `rustok-pricing` admin UI: [README](../../crates/rustok-pricing/admin/README.md)
- `rustok-commerce` admin UI: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-pages` admin UI: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-seo` admin UI: [README](../../crates/rustok-seo/admin/README.md)
- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum` admin UI: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search` admin UI: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media` admin UI: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments` admin UI: [README](../../crates/rustok-comments/admin/README.md)
- `rustok-workflow` admin UI: [README](../../crates/rustok-workflow/admin/README.md)

### Optional/storefront-поверхности

- `rustok-blog` storefront UI: [README](../../crates/rustok-blog/storefront/README.md)
- `rustok-cart` storefront UI: [README](../../crates/rustok-cart/storefront/README.md)
- `rustok-commerce` storefront UI: [README](../../crates/rustok-commerce/storefront/README.md)
- `rustok-forum` storefront UI: [README](../../crates/rustok-forum/storefront/README.md)
- `rustok-pages` storefront UI: [README](../../crates/rustok-pages/storefront/README.md)
- `rustok-pricing` storefront UI: [README](../../crates/rustok-pricing/storefront/README.md)
- `rustok-product` storefront UI: [README](../../crates/rustok-product/storefront/README.md)
- Ecommerce storefront UI routes `rustok-product` <-> `rustok-pricing`
  сохраняют навигационный context через `handle` и pricing query-поля, а locale
  продолжают брать только из host `UiRouteContext`.
- Storefront product/pricing UI показывает stable `seller_id` как seller boundary;
  `vendor` остаётся merchandising/display label и не используется как identity.
- `rustok-region` storefront UI: [README](../../crates/rustok-region/storefront/README.md)
- `rustok-search` storefront UI: [README](../../crates/rustok-search/storefront/README.md)
- `rustok-seo` остаётся `admin_only`: storefront SEO runtime живёт в `apps/storefront` и `apps/next-frontend`
  через shared SEO contract, а не через отдельный module-owned storefront package.
- Entity-specific SEO UI при этом не централизуется в `rustok-seo-admin`: page/product/blog/forum SEO
  panels принадлежат owner-модулям, а `rustok-seo-admin` остаётся cross-cutting infrastructure UI.
- Reusable owner-side SEO widgets и transport helper-ы при этом живут в support crate
  `rustok-seo-admin-support`, а не в host application code.

### Large capability/admin surfaces

- `rustok-ai` Leptos operator/admin UI: [README](../../crates/rustok-ai/admin/README.md)
- `rustok-ai` Next.js operator/admin UI: `apps/next-admin/packages/rustok-ai/`

## Что не делать

- не описывать UI package-контракт только в `docs/modules/*` без обновления
  локальных docs самого модуля;
- не дублировать module-owned UI в `apps/admin` или `apps/storefront`;
- не вводить package-local locale negotiation;
- не считать старые инструкции по установке и деплою источником истины для актуального UI
  wiring.

## Связанные документы

- [Быстрый старт по UI-пакетам](./UI_PACKAGES_QUICKSTART.md)
- [Обзор модульной платформы](./overview.md)
- [Реестр crate-ов модульной платформы](./crates-registry.md)
### Next.js admin showcase

- `rustok-blog`: `apps/next-admin/packages/blog/`
- `rustok-search`: `apps/next-admin/packages/search/`
