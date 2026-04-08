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
- `rustok-commerce` admin UI: [README](../../crates/rustok-commerce/admin/README.md)
- `rustok-pages` admin UI: [README](../../crates/rustok-pages/admin/README.md)
- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)
- `rustok-forum` admin UI: [README](../../crates/rustok-forum/admin/README.md)
- `rustok-search` admin UI: [README](../../crates/rustok-search/admin/README.md)
- `rustok-media` admin UI: [README](../../crates/rustok-media/admin/README.md)
- `rustok-comments` admin UI: [README](../../crates/rustok-comments/admin/README.md)

### Capability-owned UI

- `rustok-ai` Leptos admin UI: [README](../../crates/rustok-ai/admin/README.md)

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
