# Документация `rustok-inventory`

`rustok-inventory` — дефолтный inventory-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- inventory service logic;
- stock-related migrations;
- `InventoryModule` и `InventoryService`.

## Переходная граница

- runtime dependency: `product`;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
