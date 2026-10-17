# rustok-product-relations

Merchandising Product associations for cross-sells, up-sells, related Products, accessories, alternatives, and custom relationship types.

## Responsibilities

- Maintain directed Product associations in the `product_relations` table.
- Enforce tenant isolation, tenant-coupled Product references, no self-references, and unique source/target/type pairs.
- Maintain one complete, contiguous ordering per `(tenant_id, product_id, relation_type)`: positions are non-negative and unique, and a reorder request must list every relation in the group exactly once.
- Complete durable owner receipts in the same transaction as create, update, delete, and reorder commands, replaying their stored result for the same tenant-scoped caller key. Native FFA and Commerce GraphQL derive that owner key through the exported Relation scoping function.
- Provide indexed source-product and reverse-target lookup.

The module does **not** currently publish Product Relation lifecycle outbox events. Event publication must not be inferred from this module until a shared `rustok-events` contract is intentionally designed and registered.

## Interactions

- **`rustok-product`**: Owns the source and target Product records referenced by a relation.
- **`rustok-outbox`**: Provides the shared receipt ledger for every Relation write command; it does not currently receive Product Relation lifecycle events.
- **`rustok-commerce`**: Exposes relations through GraphQL queries and mutations.

## Entry points

- [`ProductRelationsPort`](src/ports.rs): Service and application port for relationship management.
- [`ProductRelationService`](src/services/relation_service.rs): Domain service orchestrating validation, persistence, and ordering normalization.
- [`ProductRelationsModule`](src/lib.rs): Platform module registration and migration provider.

For detailed architecture and implementation notes, see [`docs/`](docs/).
