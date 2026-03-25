# Документация `rustok-pricing`

`rustok-pricing` — дефолтный pricing-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- price-related service logic;
- pricing migrations;
- `PricingModule` и `PricingService`.

## Переходная граница

- runtime dependency: `product`;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
