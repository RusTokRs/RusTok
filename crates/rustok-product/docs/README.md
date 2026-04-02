# Документация `rustok-product`

`rustok-product` — дефолтный каталоговый подмодуль семейства `ecommerce`.

## Что сейчас внутри

- каталог товаров;
- варианты, опции, переводы и публикация;
- taxonomy-backed product tags через shared `rustok-taxonomy` и product-owned relation `product_tags`;
- product-owned migrations;
- `ProductModule` и `CatalogService`.

## Переходная граница

- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.
- Общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.
- canonical vocabulary и attach semantics для product tags живут в
  `rustok-taxonomy` + `product_tags`, а public contract использует first-class
  поле `tags` вместо legacy `metadata.tags`.
- shipping profile для товара теперь тоже имеет first-class surface в product
  DTO (`shipping_profile_slug`), хотя на этапе Phase 7 физически всё ещё
  хранится через metadata-backed contract `shipping_profile.slug`; omission
  first-class поля на write-path не должен затирать уже существующий metadata-backed
  shipping profile.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
