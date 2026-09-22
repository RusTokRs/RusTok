# Product Relations Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-product-relations` module.

## Purpose

The `rustok-product-relations` module provides dedicated relationship and merchandising management between catalog products, including:
- **Cross-sell**: complementary products (e.g. accessories, cases, protection plans).
- **Up-sell**: higher-tier or premium alternatives.
- **Related**: relevant products within or across taxonomy categories.
- **Accessory**: compatible components or attachments.
- **Alternative**: direct substitutes if a product is out of stock.

It keeps the core `rustok-product` domain pure and minimal, avoiding merchandising clutter inside the product variant lifecycle while providing high-performance indexed queries, ordered associations, and outbox event publishing.

## Responsibility Zone

The module boundary strictly owns:
- Persistence and migrations for `product_relations` table.
- Multi-tenant data isolation on all relationship operations.
- Constraint enforcement: disallow self-relations (`product_id != related_product_id`), enforce unique pair per relation type.
- Ordering and reordering within a relation type.
- Emitting transactional outbox events on creation, deletion, and reordering.

## Integration

- **`rustok-product`**: References foreign keys `product_id` and `related_product_id`.
- **`rustok-outbox`**: Emits `product_relation.created`, `product_relation.deleted`, and `product_relations.reordered`.
- **`rustok-commerce`**: Exposes relations via public GraphQL query `productRelations` and mutations `addProductRelation`, `removeProductRelation`, `reorderProductRelations`.
- **`rustok-product-admin`**: Embedded `ProductRelationsPanel` in the product management interface consumed via public transport contracts without direct database linkage (Rule 13).

## Verification

- Module unit tests: `cargo test -p rustok-product-relations` (validates self-relation rejection, uniqueness, CRUD, and reordering).
- GraphQL surface tests: `cargo test -p rustok-commerce --test graphql_surface_regression`.
- Admin UI test suite: `cargo test -p rustok-product-admin`.
- Manifest validation: `cargo xtask validate-manifest`.

## Related Documentation

- [`README.md`](../README.md): Root crate overview and entry points.
- [`docs/implementation-plan.md`](implementation-plan.md): FFA/FBA status, lifecycle invariants, and roadmap.
- [Product Catalog Documentation](../../rustok-product/docs/README.md).
