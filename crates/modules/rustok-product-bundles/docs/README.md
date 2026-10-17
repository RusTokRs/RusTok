# Product Bundles Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-product-bundles` module.

## Purpose

The module manages product bundles, kits, configurable sets, and package discounts. Store operators can:

- Create and maintain Bundles with tenant-unique slugs, fixed or flexible composition, lifecycle status, and discount configuration.
- Assemble component items with validated quantities, optional flags, positions, and item discounts; a selected Variant must belong to the selected component Product.
- Store localized Bundle names and descriptions.
- Associate a Bundle with its root catalog Product through `bundle_product_id`, or keep it independent.

`bundle_product_id` is an ownership relationship, not component membership: a Product owns zero or one Bundle, and deleting that owner cascades to its Bundle and Bundle children. A component Product can appear in many Bundle item rows; deleting a component Product or Variant cascades its affected item rows for that component membership.

## Responsibility Zone

The module boundary strictly owns:

- Persistence and migrations for `product_bundles`, `product_bundle_translations`, and `product_bundle_items`.
- Tenant-scoped Product, Variant, Bundle, and Bundle-item referential integrity, including that an item Variant belongs to its item Product.
- Slug and owner-cardinality guarantees per tenant.
- Cascading deletion of translations and Bundle items when a Bundle is removed, and of an owned Bundle when its owner Product is removed.
- Append-only upgrade safety: `m20261017_000004_enforce_bundle_tenant_integrity` backfills legacy item tenants and fails closed on orphaned, cross-tenant, wrong-Product Variant, duplicate-owner, or duplicate-position data before replacing the legacy schema transactionally.
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
