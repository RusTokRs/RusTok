# Документация `rustok-product`

`rustok-product` — дефолтный каталоговый подмодуль семейства `ecommerce`.

## Что сейчас внутри

- каталог товаров;
- варианты, опции, переводы и публикация;
- product-owned migrations;
- `ProductModule` и `CatalogService`.

## Переходная граница

- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.
- Общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
