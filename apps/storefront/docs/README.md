# Документация Leptos Storefront

Локальная документация для `apps/storefront` как Leptos SSR-host приложения витрины.

## Назначение

`apps/storefront` является Rust-first SSR-first storefront host для RusToK. Приложение рендерит shell, домашнюю страницу, generic module pages и монтирует module-owned storefront UI через manifest-driven wiring.

## Границы ответственности

- владеть Leptos storefront host и его SSR/runtime wiring;
- монтировать module-owned storefront packages из `crates/rustok-*/storefront`;
- поддерживать generic route contract для storefront-модулей;
- передавать в module-owned пакеты `UiRouteContext` и effective locale;
- не забирать внутрь host модульный business UI и модульные transport contracts.

## Runtime contract

- GraphQL transport не удаляется и остаётся обязательным внешним контрактом.
- Native Leptos `#[server]` functions используются как preferred внутренний data-layer path в SSR/hydrate runtime параллельно с GraphQL.
- CSR/WASM для Leptos storefront packages является compatibility/debug профилем. Если package должен запускаться standalone, он обязан иметь GraphQL/REST fallback и не требовать `/api/fn/*`.
- Generic storefront routes живут в семействе `/modules/{route_segment}` и `/{locale}/modules/{route_segment}`.
- Host сначала пытается использовать native `#[server]` path там, где он есть в SSR/hydrate runtime, и только потом откатывается к GraphQL.
- Module-owned storefront packages обязаны строить внутренние ссылки через `UiRouteContext::module_route_base()`, а не через hardcoded route strings.
- Module-owned storefront packages не определяют собственную locale negotiation policy; effective locale приходит из host/runtime contract.
- Module-owned Leptos storefront packages читают query/state через общий helper слой `leptos-ui-routing`,
  а не через package-local direct access к `UiRouteContext.query_value(...)`.

## Module-owned storefront surfaces

Сейчас этот contract уже используется как минимум для:

- `rustok-pages-storefront`
- `rustok-blog-storefront`
- `rustok-cart-storefront`
- `rustok-commerce-storefront`
- `rustok-pricing-storefront`
- `rustok-product-storefront`
- `rustok-region-storefront`
- `rustok-forum-storefront`
- `rustok-search-storefront`

Build-time wiring генерируется из `modules.toml` и `rustok-module.toml` через `apps/storefront/build.rs`.

## Доступ к данным

Прямые storefront server functions сейчас покрывают:

- `list-enabled-modules`
- `resolve-canonical-route`
- `storefront/seo-page-context`
- `pages/storefront-data`
- `blog/storefront-data`
- `cart/storefront-data`
- `cart/decrement-line-item`
- `cart/remove-line-item`
- `commerce/storefront-data`
- `commerce/create-payment-collection`
- `commerce/complete-checkout`
- `pricing/storefront-data`
- `pricing/storefront-data` теперь также может показывать effective pricing preview,
  если storefront route несёт optional query context (`currency`, `region_id`, `price_list_id`, `quantity`),
  и выводит pricing-owned selector активных price lists поверх этого context;
- `product/storefront-data`
- `region/storefront-data`
- `forum/storefront-data`
- `search/storefront-search`
- `search/storefront-filter-presets`
- `search/storefront-suggestions`
- `search/storefront-track-click`

GraphQL path при этом остаётся рабочим и поддерживаемым fallback-контрактом для module-owned storefront surfaces, `cart/storefront-data` теперь обслуживает cart-owned cart workspace с seller-aware delivery-group snapshot, `cart/decrement-line-item` и `cart/remove-line-item` дают безопасный line-item write-side внутри cart boundary, а `commerce/storefront-data`, `commerce/select-shipping-option`, `commerce/create-payment-collection` и `commerce/complete-checkout` обслуживают aggregate checkout workspace в `rustok-commerce/storefront`, сохраняя seller-aware shipping selection contract end-to-end.

## Canonical routing и locale

- Canonical и alias state хранится в backend/domain слоях, а не в storefront host.
- Storefront использует SEO preflight перед рендером страницы: сначала читает `SeoPageContext`, а canonical-only path остаётся fallback-веткой.
- `SeoPageContext` разделён на `route` и `document`: route-часть отвечает за redirect/canonical/hreflang, document-часть — за typed SSR head metadata.
- `storefront/seo-page-context` на SSR теперь также передаёт host `RequestContext.channel_slug` в `rustok-seo`, поэтому channel-restricted forum topics получают SEO head только в совпавшем публичном канале.
- Rust-side head serialization вынесен в `rustok-seo-render`, поэтому host не держит собственный второй renderer поверх того же SEO contract.
- Locale-prefixed routes являются основным route contract.
- Host locale normalization идёт через shared `rustok_core::normalize_locale_tag`, а не через package-local правила.
- Legacy query-based locale fallback допускается только как backward-compatible path.

## Взаимодействия

- `apps/server` предоставляет GraphQL и Leptos server-function surfaces.
- `crates/rustok-*` публикуют module-owned storefront packages и runtime transport contracts.
- `apps/next-frontend` идёт параллельным storefront host и должен сохранять parity на уровне контрактов, а не на уровне буквального устройства кода.
- `leptos-ui-routing` выступает общим Leptos route/query plumbing и для admin, и для storefront;
  storefront host не должен дублировать этот слой отдельным Rust helper crate.

## Проверка

- `npm.cmd run verify:storefront:routes`
- storefront-specific точечные smoke/contract прогоны по module-owned surfaces
- при изменении manifest wiring сверяться с `docs/modules/manifest.md`

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Storefront architecture notes](../../../docs/UI/storefront.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Карта документации](../../../docs/index.md)
