# Документация `rustok-region`

`rustok-region` — дефолтный region-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- схема `regions`;
- `RegionModule` и `RegionService`;
- region boundary для currency/country/tax policy;
- дефолтный lookup региона по `region_id` или стране.

## Архитектурная граница

- модуль владеет таблицей `regions` и больше не прячется внутри `pricing`;
- модуль не владеет tenant locales: они остаются platform-core данными;
- locale/currency orchestration живет в umbrella `rustok-commerce`, который связывает `regions` с tenant locale policy.

## Связанные документы

- [README crate](../README.md)
- [План umbrella `commerce`](../../rustok-commerce/docs/implementation-plan.md)
