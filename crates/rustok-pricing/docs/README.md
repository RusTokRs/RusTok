# Документация `rustok-pricing`

`rustok-pricing` — дефолтный pricing-подмодуль семейства `ecommerce`.

## Назначение

- price-related service logic;
- pricing migrations;
- `PricingModule` и `PricingService`.

## Зона ответственности

- runtime dependency: `product`;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate pricing
- cargo xtask module test pricing
- targeted commerce tests для pricing-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
