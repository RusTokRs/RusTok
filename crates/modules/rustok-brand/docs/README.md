# Brand Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-brand` module.

## Purpose

The `rustok-brand` module manages brand and manufacturer entities in RusToK. It enables store operators to:
- Create and maintain brands with custom slugs, website URLs, and active flags.
- Attach media assets (logo, hero banners) for brand landing pages and store presentation.
- Store rich bilingual/multilingual localized names and descriptions.
- Associate products with one or more brands (supporting collaborative and co-branded releases).
- Provide clean SEO landing targets (`/brands/{slug}`) without cluttering the product entity.

## Responsibility Zone

The module boundary strictly owns:
- Persistence and migrations for `brands`, `brand_translations`, and `brand_products`.
- Multi-tenant data isolation on all operations (requiring non-nil `tenant_id`).
- Slug validation and uniqueness guarantees per tenant.
- Cascading deletion of translations and product bindings when a brand is removed.
- Transactional outbox event publishing (`brand.created`, `brand.updated`, `brand.deleted`).

## Integration

- **`rustok-product`**: References product IDs in `brand_products`.
- **`rustok-media`**: References media assets for logo and banner presentation.
- **`rustok-outbox`**: Emits domain events for search indexing, cache invalidation, and external notifications.
- **`rustok-commerce`**: Exposes brands through GraphQL queries and mutations.
- **`rustok-product-admin`**: Embedded brand selector and editor panels.

## Verification

- Unit tests: `cargo test -p rustok-brand` (validates CRUD, translations upsert, slug uniqueness, and product assignment).
- GraphQL surface tests: `cargo test -p rustok-commerce --test graphql_surface_regression`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module contract validation: `cargo xtask module validate brand`.

## Related Documentation

- [`README.md`](../README.md): Root crate overview and entry points.
- [`docs/implementation-plan.md`](implementation-plan.md): FFA/FBA status, invariants, and roadmap.
- [Product Catalog Documentation](../../rustok-product/docs/README.md).
