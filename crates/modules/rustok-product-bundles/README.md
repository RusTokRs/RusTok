# rustok-product-bundles

Product bundles, kits, configurable sets, and package discounts for the RusToK platform.

## Purpose

The `rustok-product-bundles` module manages product bundles and kits (fixed composition or flexible/configurable sets), bundle-level and item-level discounts, localized names and descriptions, and product/variant item associations.

## Responsibility Zone

The module boundary strictly owns:
- Persistence and migrations for `product_bundles`, `product_bundle_translations`, and `product_bundle_items` tables.
- Multi-tenant data isolation on all bundle queries and mutations.
- Unique slug constraints per tenant (`UNIQUE (tenant_id, slug)`).
- Bundle lifecycle (draft, active, archived) and bundle types (fixed, flexible).
- Localized translations lifecycle for bundle names and descriptions.
- Bundle item composition with quantity, optionality, position, and item-specific discounts.
- Emitting transactional outbox events on bundle lifecycle events (`bundle.created`, `bundle.updated`, `bundle.deleted`).

## Integration

- **`rustok-product`**: References catalog products and variants in bundle items.
- **`rustok-outbox`**: Emits `bundle.created`, `bundle.updated`, and `bundle.deleted` events for inventory and pricing consumers.
- **`rustok-product-bundles-admin`**: Dedicated FFA/FBA administration UI package.
- **`rustok-commerce`**: Exposes bundles via GraphQL queries and mutations.

## Verification

- Module unit tests: `cargo test -p rustok-product-bundles`.
- Admin UI tests: `cargo test -p rustok-product-bundles-admin --features ssr`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module publish contract: `cargo xtask module validate product_bundles`.

## Related Documentation

- [`docs/implementation-plan.md`](docs/implementation-plan.md): FFA/FBA status, invariants, and roadmap.
