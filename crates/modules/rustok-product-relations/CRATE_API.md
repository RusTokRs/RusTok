# rustok-product-relations / CRATE_API

## Role

`rustok-product-relations` owns directed merchandising associations between catalog products:
cross-sells, up-sells, related products, accessories, alternatives, and tenant-defined custom
relationships.

Persistence entities are **crate-private**. Consumers interact exclusively through owner DTOs,
services, or the application port. Direct cross-crate SeaORM table access is forbidden.

## Public surface

Deliberate public modules and exports:

- `dto` — owner command, input, and response data contracts:
  - `ProductRelationDto`
  - `CreateProductRelationInput`, `UpdateProductRelationInput`, `ReorderProductRelationsInput`
  - `RelationType`, `MAX_RELATION_TYPE_LEN`
- `error` — owner error contract:
  - `ProductRelationError`, `ProductRelationResult`
- `ports` — cross-boundary application port:
  - `ProductRelationsPort`
- `services` — domain use cases, pessimistic concurrency, and durable receipts:
  - `ProductRelationService`
  - `ProductRelationCommandContext`, `ProductRelationCommandError`, `relation_command_error`
  - Constants: `PRODUCT_RELATION_OWNER_SLUG`, `CREATE_RELATION_OPERATION`,
    `UPDATE_RELATION_OPERATION`, `DELETE_RELATION_OPERATION`, `REORDER_RELATIONS_OPERATION`
- `module` — platform module lifecycle and migration source:
  - `ProductRelationsModule`
- Ingress idempotency:
  - `scoped_caller_idempotency_key`, `RELATION_CALLER_IDEMPOTENCY_NAMESPACE`

`entities` and `migrations` are crate-private.

## Primary entry points

- [`ProductRelationsPort`](src/ports.rs): Primary trait for reading and mutating product relations.
- [`ProductRelationService`](src/services/relation_service.rs): Domain application service.
- [`ProductRelationsModule`](src/module.rs): RusToK platform module and migration registration.
- [`scoped_caller_idempotency_key`](src/lib.rs): Derives stable owner receipt identity for ingress callers.

## Ordering and reordering contract

1. **Zero-based contiguous ordering**: Positions within a `(tenant_id, product_id, relation_type)`
   group are contiguous integers `0..N` without gaps.
2. **Atomic insertion**: When inserting with an explicit position, higher positions are shifted up.
   When position is omitted, the relation is appended at position `N`.
3. **Atomic deletion**: When deleting a relation, subsequent items are shifted down to fill the gap.
4. **Complete permutation on reorder**: A reorder request must contain every relation ID in the group
   exactly once. Partial or duplicate reorders are rejected as `IncompleteReorder` or
   `DuplicateRelationInReorder`.

## Concurrency and locking contract

1. **Deadlock prevention**: Multi-product operations acquire exclusive row locks on catalog products
   ordered by UUID (`left < right`).
2. **TOCTOU mitigation**: The relation is reloaded inside the transaction after acquiring the source
   product lock, eliminating race conditions with concurrent deletes.

## Idempotency and receipts contract

1. Every write mutation (`create`, `update`, `delete`, `reorder`) accepts a caller-supplied
   idempotency key.
2. The owner completes a durable receipt in the outbox ledger inside the same database transaction.
3. Subsequent requests with the same key replay the stored result, rejecting mismatched payloads
   as conflicts.
