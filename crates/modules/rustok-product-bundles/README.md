# rustok-product-bundles

Product bundles, kits, configurable sets, and package discounts for the RusToK platform.

## Purpose

The `rustok-product-bundles` module manages product bundles and kits (fixed composition or flexible/configurable sets), bundle-level and item-level discounts, localized names and descriptions, and product/variant item associations.

See [CRATE_API.md](CRATE_API.md) for the verified public crate API specification and platform capability bindings.

## Responsibility Zone

The module boundary strictly owns:

- Persistence and migrations for `product_bundles`, `product_bundle_translations`, and `product_bundle_items`.
- Tenant-scoped reads and writes, including tenant-coupled Bundle, Product, and Variant references at the persistence boundary; a selected item Variant is constrained to its selected item Product.
- Append-only upgrade safety: `m20261017_000004_enforce_bundle_tenant_integrity` backfills legacy item tenants and fails closed on orphaned, cross-tenant, wrong-Product Variant, duplicate-owner, or duplicate-position data before replacing the legacy schema transactionally.
- A unique slug per tenant (`UNIQUE (tenant_id, slug)`).
- A zero-or-one Product-owned Bundle association: `bundle_product_id` identifies the Product that owns the Bundle aggregate. Deleting that owner deletes its Bundle and Bundle children; component membership is represented only by `product_bundle_items`, and a component Product can belong to many Bundles.
- Bundle lifecycle values (`draft`, `active`, `archived`) and bundle types (`fixed`, `flexible`).
- Validated bundle and item discount ranges, item quantities, and item positions.
- Localized translations lifecycle for bundle names and descriptions.
- Durable idempotency receipts for every Bundle mutation (create, update, delete, add item, and remove item); Native FFA and Commerce GraphQL derive one owner key through the exported Bundle scoping function.
- Database-enforced unique-claim races are surfaced as terminal owner conflicts rather than storage outages, so receipt retries replay a stable outcome.

The module does **not** currently publish Bundle lifecycle outbox events. Adding such events requires an intentional registered `rustok-events` contract (event variants, schemas, and contract digests); it is not a local outbox-row convention.

## Integration

- **`rustok-product`**: Provides tenant-scoped catalog Product and Variant references for owners and component items.
- **`rustok-product-bundles-admin`**: Dedicated FFA/FBA administration UI package.
- **`rustok-commerce`**: Exposes Bundle queries and receipt-bound mutations via GraphQL.
- **`rustok-outbox`**: Supplies durable receipt infrastructure; it does not currently carry Bundle lifecycle events.

## Verification

- Module unit tests: `cargo test -p rustok-product-bundles`.
- Admin UI tests: `cargo test -p rustok-product-bundles-admin --features ssr`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module publish contract: `cargo xtask module validate product_bundles`.

## Related Documentation

- [`docs/implementation-plan.md`](docs/implementation-plan.md): FFA/FBA status, invariants, and roadmap.
