# rustok-product-relations

Product relations, cross-sells, up-sells, accessories, and merchandising associations for the RusToK platform.

## Purpose

Provides dedicated management and retrieval of relationships between products (such as cross-sells, up-sells, related products, accessories, and alternatives) without polluting the core `rustok-product` catalog entity with merchandising logic.

## Responsibilities

- Maintain bidirectional and directed product associations (`product_relations` table).
- Enforce relation constraints (disallow self-referencing, multi-tenant isolation, unique relation pairs per type).
- Emit outbox events on relation changes (`ProductRelationCreated`, `ProductRelationDeleted`, `ProductRelationsReordered`).
- Provide fast indexed querying by source product and relation type, as well as reverse lookup.

## Interactions

- **`rustok-product`**: References source and target product IDs.
- **`rustok-outbox`**: Publishes transactional outbox events for downstream search indices and cache invalidation.
- **`rustok-commerce`**: Exposes relations via GraphQL API queries and mutations.

## Entry points

- [`ProductRelationsPort`](src/ports.rs): Service and application port for relationship management.
- [`ProductRelationService`](src/services/relation_service.rs): Domain service orchestrating validation, persistence, and outbox emission.
- [`ProductRelationsModule`](src/lib.rs): Platform module registration and migration provider.

For detailed architecture and implementation notes, see [`docs/`](docs/).
