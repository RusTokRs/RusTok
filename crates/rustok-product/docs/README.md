# Документация `rustok-product`

`rustok-product` — дефолтный каталоговый подмодуль семейства `ecommerce`.

## Назначение

- каталог товаров;
- варианты, опции, переводы и публикация;
- taxonomy-backed product tags через shared `rustok-taxonomy` и product-owned relation `product_tags`;
- product-owned migrations;
- `ProductModule`, `CatalogService`, module-owned admin UI пакет `rustok-product/admin` и module-owned storefront UI пакет `rustok-product/storefront`.

## Зона ответственности

- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.
- storefront read-side для published catalog уже живёт в `rustok-product/storefront` и использует native Leptos server functions поверх `CatalogService`, сохраняя GraphQL storefront contract как fallback.
- product CRUD в admin UI уже вынесен из `rustok-commerce-admin`
  в module-owned route `product`, но transport-контракт для этих форм по-прежнему
  приходит через umbrella `rustok-commerce` GraphQL surface;
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

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate product
- cargo xtask module test product
- targeted commerce tests для product-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [README admin UI](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
