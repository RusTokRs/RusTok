# rustok-product

## Purpose

`rustok-product` is the default catalog submodule of the `Ecommerce` family.

## Responsibilities

- Product entities, translations, options, variants, and product-owned migrations.
- Product write-side services and publication lifecycle.
- Product module metadata for runtime registration.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs/entities/errors.
- Depends on `rustok-outbox` and `rustok-events` for transactional event publishing.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.

## Entry points

- `ProductModule`
- `CatalogService`

See also `docs/README.md`.
