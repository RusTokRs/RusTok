# Документация Leptos Storefront

Локальная документация для `apps/storefront` как Leptos SSR-host приложения витрины.

## Назначение

`apps/storefront` является Rust-first storefront host для RusToK. Приложение рендерит shell, домашнюю страницу, generic module pages и монтирует module-owned storefront UI через manifest-driven wiring.

## Границы ответственности

- владеть Leptos storefront host и его SSR/runtime wiring;
- монтировать module-owned storefront packages из `crates/rustok-*/storefront`;
- поддерживать generic route contract для storefront-модулей;
- передавать в module-owned пакеты `UiRouteContext` и effective locale;
- не забирать внутрь host модульный business UI и модульные transport contracts.

## Runtime contract

- GraphQL transport не удаляется и остаётся обязательным внешним контрактом.
- Native Leptos `#[server]` functions используются как внутренний data-layer path параллельно с GraphQL.
- Generic storefront routes живут в семействе `/modules/{route_segment}` и `/{locale}/modules/{route_segment}`.
- Host сначала пытается использовать native `#[server]` path там, где он есть, и только потом откатывается к GraphQL.
- Module-owned storefront packages обязаны строить внутренние ссылки через `UiRouteContext::module_route_base()`, а не через hardcoded route strings.
- Module-owned storefront packages не определяют собственную locale negotiation policy; effective locale приходит из host/runtime contract.

## Module-owned storefront surfaces

Сейчас этот contract уже используется как минимум для:

- `rustok-pages-storefront`
- `rustok-blog-storefront`
- `rustok-commerce-storefront`
- `rustok-forum-storefront`
- `rustok-search-storefront`

Build-time wiring генерируется из `modules.toml` и `rustok-module.toml` через `apps/storefront/build.rs`.

## Доступ к данным

Прямые storefront server functions сейчас покрывают:

- `list-enabled-modules`
- `resolve-canonical-route`
- `pages/storefront-data`
- `blog/storefront-data`
- `commerce/storefront-data`
- `forum/storefront-data`
- `search/storefront-search`
- `search/storefront-filter-presets`
- `search/storefront-suggestions`
- `search/storefront-track-click`

GraphQL path при этом остаётся рабочим и поддерживаемым fallback-контрактом.

## Canonical routing и locale

- Canonical и alias state хранится в backend/domain слоях, а не в storefront host.
- Storefront использует canonical preflight перед рендером страницы.
- Locale-prefixed routes являются основным route contract.
- Legacy query-based locale fallback допускается только как backward-compatible path.

## Взаимодействия

- `apps/server` предоставляет GraphQL и Leptos server-function surfaces.
- `crates/rustok-*` публикуют module-owned storefront packages и runtime transport contracts.
- `apps/next-frontend` идёт параллельным storefront host и должен сохранять parity на уровне контрактов, а не на уровне буквального устройства кода.

## Проверка

- `npm.cmd run verify:storefront:routes`
- storefront-specific точечные smoke/contract прогоны по module-owned surfaces
- при изменении manifest wiring сверяться с `docs/modules/manifest.md`

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Storefront architecture notes](../../../docs/UI/storefront.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
- [Карта документации](../../../docs/index.md)
