# Product Bundles Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-product-bundles` module.

## Purpose

The `rustok-product-bundles` module manages product bundles, kits, configurable sets, and package discounts in RusToK. It enables store operators to:
- Create and maintain bundles with custom slugs, bundle types (fixed kit vs. flexible set), active statuses, and discount configurations (percentage or fixed discount).
- Assemble bundle items with individual quantities, optional flags, and custom item discounts.
- Store bilingual/multilingual localized names and descriptions per bundle.
- Associate a bundle with a root catalog product (`bundle_product_id`) or treat it as an independent bundle set.

## Responsibility Zone

The module boundary strictly owns:
- Persistence and migrations for `product_bundles`, `product_bundle_translations`, and `product_bundle_items`.
- Multi-tenant data isolation on all operations (requiring non-nil `tenant_id`).
- Slug validation and uniqueness guarantees per tenant.
- Cascading deletion of translations and bundle items when a bundle is removed.
- Transactional outbox event publishing (`bundle.created`, `bundle.updated`, `bundle.deleted`).

## Integration

- **`rustok-product`**: References product IDs and variant IDs in `product_bundle_items` and `product_bundles`.
- **`rustok-pricing`**: Discount application on bundle items and aggregate bundle totals.
- **`rustok-outbox`**: Emits domain events for inventory allocation, search indexing, and cart/checkout validation.
- **`rustok-commerce`**: Exposes bundle queries and mutations via GraphQL.
- **`rustok-product-bundles-admin`**: Dedicated Leptos FFA admin package for bundle management.

## Verification

- Unit tests: `cargo test -p rustok-product-bundles` (validates CRUD, item management, translations, and discount calculations).
- Admin package check: `cargo check -p rustok-product-bundles-admin`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module contract validation: `cargo xtask module validate product_bundles`.

## Related Documentation

- [`README.md`](../README.md): Root crate overview and entry points.
- [`docs/implementation-plan.md`](implementation-plan.md): FFA/FBA status, invariants, and roadmap.
- [Product Catalog Documentation](../../rustok-product/docs/README.md).
