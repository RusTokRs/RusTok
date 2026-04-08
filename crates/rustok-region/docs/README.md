# Документация `rustok-region`

`rustok-region` — дефолтный region-подмодуль семейства `ecommerce`.

## Назначение

- схема `regions`;
- `RegionModule` и `RegionService`;
- region boundary для currency/country/tax policy;
- дефолтный lookup региона по `region_id` или стране.

## Зона ответственности

- модуль владеет таблицей `regions` и больше не прячется внутри `pricing`;
- модуль не владеет tenant locales: они остаются platform-core данными;
- locale/currency orchestration живет в umbrella `rustok-commerce`, который связывает `regions` с tenant locale policy.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate region
- cargo xtask module test region
- targeted commerce tests для region-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [План umbrella `commerce`](../../rustok-commerce/docs/implementation-plan.md)
