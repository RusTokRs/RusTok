# Документация `rustok-region`

`rustok-region` — дефолтный region-подмодуль семейства `ecommerce`.

## Назначение

- схема `regions`;
- `RegionModule` и `RegionService`;
- region boundary для country/currency/tax baseline;
- typed `tax_provider_id` как region-owned baseline hook для выбора tax provider;
- optional channel-scoped override map в `metadata.channel_tax_provider_ids` (string или object `{provider_id|provider}`) используется cart/tax runtime только при наличии `channel_id`;
- module-owned admin UI для region CRUD;
- module-owned storefront UI для public region discovery;
- дефолтный lookup региона по `region_id` или стране.

## Зона ответственности

- модуль владеет таблицей `regions` и baseline-политикой по странам, валюте и tax flags;
- модуль не владеет tenant locales: они остаются platform-core данными;
- channel-specific tax-provider override map остаётся compatibility metadata-contract и не заменяет typed baseline `tax_provider_id`;
- locale/currency orchestration над baseline по-прежнему живёт в umbrella `rustok-commerce`, который связывает `regions` с tenant locale policy;
- operator-facing admin CRUD теперь публикуется самим модулем через `rustok-region/admin`, а не через aggregate `commerce`.
- public storefront read-side теперь тоже публикуется самим модулем через `rustok-region/storefront`, а не через aggregate storefront route.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- storefront transport для region discovery по-прежнему публикуется через `rustok-commerce`;
- storefront route `/modules/regions` теперь публикуется самим модулем через `[provides.storefront_ui]`, сохраняя GraphQL transport параллельным fallback-контрактом;
- admin UI подключается host-приложением `apps/admin` через manifest-driven `[provides.admin_ui]`;
- Leptos admin/storefront packages используют native `#[server]` functions как default internal data layer и читают effective locale из `UiRouteContext.locale`; storefront route/tax/country summary formatting, selected-region resolution и error status/view-model mapping вынесены в framework-agnostic `storefront/src/core.rs`, а native/GraphQL transport paths разделены через `storefront/src/transport/` с typed fallback error envelope.

## Проверка

- `cargo xtask module validate region`
- `cargo xtask module test region`
- `cargo check -p rustok-region-admin --lib`
- `cargo check -p rustok-region-storefront --lib`
- targeted commerce tests для storefront region transport при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [План реализации `rustok-region`](./implementation-plan.md)
- [План umbrella `commerce`](../../rustok-commerce/docs/implementation-plan.md)
