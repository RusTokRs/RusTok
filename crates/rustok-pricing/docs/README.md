# Документация `rustok-pricing`

`rustok-pricing` — дефолтный pricing-подмодуль семейства `ecommerce`.

## Назначение

- price-related service logic;
- pricing migrations;
- `PricingModule` и `PricingService`;
- module-owned admin UI пакет `rustok-pricing/admin` для price visibility,
  sale markers и currency coverage inspection.
- module-owned storefront UI пакет `rustok-pricing/storefront` для public pricing
  discovery, currency coverage и sale-marker visibility.

## Зона ответственности

- runtime dependency: `product`;
- модуль владеет pricing boundary и операторской read-side UI-поверхностью для цен;
- модуль теперь владеет и публичной storefront read-side pricing-поверхностью,
  которая строит pricing atlas поверх published catalog и variant-level prices;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`, а dedicated
  pricing write transport ещё не вынесен в отдельный module-owned surface;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу
  без возврата ответственности в umbrella `rustok-commerce`;
- transport и GraphQL пока публикуются через `rustok-commerce`, а pricing-owned admin/storefront
  UX уже публикуется через `rustok-pricing/admin` и `rustok-pricing/storefront`;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce`
  и соседними split-модулями.

## Проверка

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- targeted commerce tests для pricing-домена при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
