# rustok-brand

Brand catalog, manufacturers, media presentation, and product associations for the RusToK platform.

## Purpose

The `rustok-brand` module provides dedicated brand/manufacturer management (slug, logo media ID, banner media ID, localized names and descriptions, website URL, and custom metadata) and associates products with their respective brands.

See [CRATE_API.md](CRATE_API.md) for the verified public crate API specification and platform capability bindings.

## Responsibility Zone

The module boundary strictly owns:
- Persistence and migrations for `brands`, `brand_translations`, and `brand_products` tables.
- Multi-tenant data isolation on all brand queries and mutations.
- Unique slug constraints per tenant (`UNIQUE (tenant_id, slug)`).
- Localized translations lifecycle for brand names and descriptions.
- Product-to-brand association with primary brand designation.

## Integration

- **`rustok-product`**: Associates catalog products with brands via `brand_products`.
- **`rustok-media`**: References media assets (`logo_media_id`, `banner_media_id`).
- **`rustok-commerce`**: Exposes brands via GraphQL queries and mutations.
- **`rustok-product-admin`**: Embedded brand selector and management in catalog administration.

## Verification

- Module contracts and SQLite owner scenarios: `cargo test -p rustok-brand --lib --test brand`.
- PostgreSQL profile: see the [owner verification plan](docs/implementation-plan.md#verification).
- GraphQL surface tests: `cargo test -p rustok-commerce --test graphql_surface_regression`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module layout and wiring: `cargo xtask module lint brand` (see outstanding admin findings in the owner plan).

## Related Documentation

- [`docs/README.md`](docs/README.md): Detailed module runtime contract.
- [`docs/implementation-plan.md`](docs/implementation-plan.md): FFA/FBA status, invariants, and roadmap.
- [Product Catalog Documentation](../rustok-product/docs/README.md).
