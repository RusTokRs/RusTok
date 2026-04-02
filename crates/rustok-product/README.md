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
  temporary metadata-backed shipping profile contract.
- Product module metadata for runtime registration.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs/entities/errors.
- Depends on `rustok-taxonomy` for shared scope-aware tag dictionary while keeping `product_tags`
  module-owned.
- Depends on `rustok-outbox` and `rustok-events` for transactional event publishing.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.

## Entry points

- `ProductModule`
- `CatalogService`

See also `docs/README.md`.
