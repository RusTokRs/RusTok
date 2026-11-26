# Brand Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-brand` module.

## Purpose

The `rustok-brand` module manages brand and manufacturer entities in RusToK. It enables store operators to:
- Create and maintain brands with custom slugs, website URLs, and active flags.
- Attach media assets (logo, hero banners) for brand landing pages and store presentation.
- Store rich bilingual/multilingual localized names and descriptions.
- Scope brands to categories via `brand_categories` with featured priority.
- Provide clean SEO landing targets (`/brands/{slug}`) without cluttering the product entity.

## Responsibility Zone

The module boundary strictly owns:
- Persistence and migrations for `brands`, `brand_translations`, and `brand_categories`.
- Multi-tenant data isolation on all operations (requiring non-nil `tenant_id`).
- Slug validation and uniqueness guarantees per tenant.
- Cascading deletion of translations and category bindings when a brand is removed.

## File map

The table documents Brand's current backend layout. The
[canonical file placement](../../../../docs/backend/module-backend-implementation.md#file-placement-for-reference-capabilities)
remains applicable.

| File | Responsibility |
| --- | --- |
| `src/lib.rs`, `src/module.rs` | Public facade and platform lifecycle, permissions, and migration registration |
| `src/dto/brand.rs` | Language-neutral brand fields, explicit translation inputs, and read DTOs |
| `src/ports/brand.rs` | Owner operation contract (`BrandPort`, `BrandReadPort`) consumed by native admin, `rustok-product`, and `rustok-commerce` |
| `src/services/brand/mod.rs` | Port implementation and service composition |
| `src/services/brand/commands.rs` | Owner writes and transaction boundaries; save localized rows in the same transaction as the base row |
| `src/services/brand/queries.rs` | Tenant-scoped reads, filters, and paging |
| `src/services/brand/repository.rs` | Private row reads and shared mapping; locale fallback is confined to the returned read projection |
| `src/entities/{brand,brand_translation,brand_category}.rs` | Private table mappings. Consumers use `BrandPort` / `BrandReadPort` and owner DTOs |
| `src/migrations/mod.rs` | Shared owner migration list for runtime and tests; preserve shipped migration identities |
| `src/tests/contract.rs`, `tests/brand.rs` | Crate wiring checks and public behavior on the shipped schema |

Commands load the updated localized snapshot before committing. If the
snapshot cannot be read, the same transaction rolls back the write. Pass the
active transaction to private persistence helpers when adding a related write.

## Integration

- **`rustok-product`**: Owns canonical product-to-brand bindings in `product_brands` and resolves/validates brands via `BrandReadPort`.
- **`rustok-media`**: References media assets for logo and banner presentation.
- **`rustok-commerce`**: Exposes brands through GraphQL queries and mutations.
- **`rustok-product-admin`**: Embedded brand selector and editor panels.

Brand does not currently write transactional outbox events. Cache, search and
index consumers must not rely on a Brand event until the owner publishes a
typed contract and verifies its commit, rollback, and delivery. Media fields
currently carry IDs; their presence is not evidence of retention or ownership
validation. The [platform integration guide](../../../../docs/backend/module-backend-implementation.md#connecting-platform-capabilities)
describes how to connect these capabilities when an owner workflow needs them.

## Verification

- Module contracts and SQLite owner scenarios: `cargo test -p rustok-brand --lib --test brand`.
- The [owner verification plan](implementation-plan.md#verification) lists the separate PostgreSQL invocation and its prerequisites.
- GraphQL surface tests: `cargo test -p rustok-commerce --test graphql_surface_regression`.
- Manifest validation: `cargo xtask validate-manifest`.
- Module layout and wiring: `cargo xtask module lint brand` (see outstanding admin findings in the owner plan).

## Related Documentation

- [`README.md`](../README.md): Root crate overview and entry points.
- [`docs/implementation-plan.md`](implementation-plan.md): FFA/FBA status, invariants, and roadmap.
- [Product Catalog Documentation](../../rustok-product/docs/README.md).
