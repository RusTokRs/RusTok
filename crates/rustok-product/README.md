# rustok-product

## Purpose

`rustok-product` is the default catalog submodule of the `Ecommerce` family.

## Responsibilities

- Product entities, translations, options, variants, and product-owned migrations.
- Product-owned relation storage for taxonomy-backed tags (`product_tags`).
- Product write-side services and publication lifecycle.
- Product-side synchronization of first-class `tags` contract fields with the
  taxonomy-backed dictionary.
- Product-side normalization of first-class `shipping_profile_slug` onto the
  temporary metadata-backed shipping profile contract, without erasing an
  existing metadata-backed profile when the typed field is omitted.
- Product-side split and locale-aware resolution of Flex attached custom-field
  values, using shared `flex` attached localized storage while preserving
  non-Flex operational metadata in `products.metadata`.
- Publish a module-owned Leptos admin UI package in `admin/` for catalog CRUD,
  publication lifecycle, and shipping-profile selection.
- Product module metadata for runtime registration.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs/entities/errors.
- Depends on `flex` for shared attached localized-value storage helpers used by
  product custom-field multilingual flows.
- Depends on `rustok-taxonomy` for shared scope-aware tag dictionary while keeping `product_tags`
  module-owned.
- Depends on `rustok-outbox` and `rustok-events` for transactional event publishing.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Consumed by `apps/admin` through manifest-driven module UI composition.

## Entry points

- `ProductModule`
- `CatalogService`
- `admin::ProductAdmin`

See also `docs/README.md`.
