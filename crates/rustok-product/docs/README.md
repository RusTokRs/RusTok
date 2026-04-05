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
- shipping profile для товара и варианта теперь имеет first-class typed surface в
  product DTO (`shipping_profile_slug`) и typed persistence в
  `products.shipping_profile_slug` / `product_variants.shipping_profile_slug`; metadata-backed
  `shipping_profile.slug` остаётся только backward-compatible формой нормализации для старых
  read/write-path consumer'ов.
- effective shipping profile для deliverability теперь разрешается как
  `variant.shipping_profile_slug -> product.shipping_profile_slug -> default`, а omission
  first-class поля на write-path не должен затирать уже существующую typed binding/compatibility
  normalization.
- transport-level validation для `shipping_profile_slug` теперь живёт в фасаде
  `rustok-commerce` и проверяет ссылку против active shipping profiles из typed
  registry `shipping_profiles`, чтобы product write-path не принимал произвольные slug'и.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
