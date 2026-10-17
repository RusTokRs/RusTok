# Product Relations Module Documentation

This document describes the runtime contract, boundaries, and architecture of the `rustok-product-relations` module.

## Purpose

The module holds directed merchandising associations between catalog Products:

- **Cross-sell**: complementary products commonly bought together.
- **Up-sell**: premium or upgraded alternatives.
- **Related**: relevant products within or across taxonomy categories.
- **Accessory**: compatible components or attachments.
- **Alternative**: direct substitutes if a Product is out of stock.

It keeps the core `rustok-product` domain focused while providing indexed, ordered association queries.

## Responsibility Zone

The module boundary strictly owns:

- Persistence and migrations for `product_relations`.
- Tenant isolation and tenant-coupled foreign keys to source and target Products.
- Constraint enforcement: no self-relations, one source/target/type pair, and one position per source/type group.
- Complete ordering and reordering within a relation type. Valid stored positions are contiguous and non-negative; reorder input must contain the complete group exactly once.
- Durable owner receipts for create, update, delete, and reorder. The command response is stored in the same local transaction as the relation mutation and replays for the same tenant-scoped caller key.

The module does **not** currently emit transactional lifecycle outbox events. Names such as `product_relation.created`, `product_relation.deleted`, and `product_relations.reordered` are not an implemented event contract. Any future event publication requires a shared registered `rustok-events` design.

## Integration

- **`rustok-product`**: Supplies tenant-scoped source and target Product records.
- **`rustok-outbox`**: Supplies the shared durable receipt ledger for all Relation writes; it does not carry Product Relation lifecycle events today.
- **`rustok-commerce`**: Exposes relations via `productRelations`, `addProductRelation`, `removeProductRelation`, and `reorderProductRelations`; every mutation carries a required idempotency key.
- **`rustok-product-relations-admin`**: Provides the dedicated Leptos FFA administration package.

## Verification

- Module unit tests: `cargo test -p rustok-product-relations` (self-relation rejection, Product lock fixture, uniqueness, CRUD, complete ordering, and reorder validation).
- GraphQL surface tests: `cargo test -p rustok-commerce --test graphql_surface_regression`.
- Admin UI test suite: `cargo test -p rustok-product-relations-admin`.
- Manifest validation: `cargo xtask validate-manifest`.

## Related Documentation

- [`README.md`](../README.md): Root crate overview and entry points.
- [`implementation-plan.md`](implementation-plan.md): FFA/FBA status, invariants, and roadmap.
- [Product Catalog Documentation](../../rustok-product/docs/README.md).
