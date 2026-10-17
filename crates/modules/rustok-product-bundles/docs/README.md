# Product Bundles Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-product-bundles` module.

## Purpose

The module manages product bundles, kits, configurable sets, and package discounts. Store operators can:

- Create and maintain Bundles with tenant-unique slugs, fixed or flexible composition, lifecycle status, and discount configuration.
- Assemble component items with validated quantities, optional flags, positions, and item discounts.
- Store localized Bundle names and descriptions.
- Associate a Bundle with its root catalog Product through `bundle_product_id`, or keep it independent.

`bundle_product_id` is an ownership relationship, not component membership: a Product owns zero or one Bundle, while a component Product can appear in many Bundle item rows.

## Responsibility Zone

The module boundary strictly owns:

- Persistence and migrations for `product_bundles`, `product_bundle_translations`, and `product_bundle_items`.
- Tenant-scoped Product, Variant, Bundle, and Bundle-item referential integrity.
- Slug and owner-cardinality guarantees per tenant.
- Cascading deletion of translations and Bundle items when a Bundle is removed.
- Receipt-bound create, update, delete, add-item, and remove-item commands.

The module does not currently publish `bundle.created`, `bundle.updated`, or `bundle.deleted` lifecycle events. Those event names must not be consumed as an established contract. Lifecycle publication requires a separate accepted change to the shared `rustok-events` registry, schema, and digest artifacts.

## Integration

- **`rustok-product`**: Supplies tenant-scoped Product and Variant references for Bundle owners and component items.
- **`rustok-pricing`**: Applies Bundle and item discounts when pricing consumes this data.
- **`rustok-outbox`**: Supplies durable command receipt infrastructure; it does not currently emit Bundle lifecycle events.
- **`rustok-commerce`**: Exposes Bundle queries and receipt-bound mutations via GraphQL.
- **`rustok-product-bundles-admin`**: Dedicated Leptos FFA admin package for Bundle management.

## Verification

- Unit tests: `cargo test -p rustok-product-bundles` (CRUD, ownership semantics, item management, translations, and validation).
- Admin package check: `cargo check -p rustok-product-bundles-admin`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module contract validation: `cargo xtask module validate product_bundles`.

## Related Documentation

- [`README.md`](../README.md): Root crate overview and entry points.
- [`docs/implementation-plan.md`](implementation-plan.md): FFA/FBA status, invariants, and roadmap.
- [Product Catalog Documentation](../../rustok-product/docs/README.md).
