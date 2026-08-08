# Product schema write boundary recheck — 2026-08-08

## Scope

This source-only continuation follows Product schema-write capability publication, mounted consumer cutover, mandatory GraphQL caller identity, and the first durable attribute-create receipt slice. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; its combined Product schema-write/idempotency item remains open while update-style owner replay semantics are defined and implemented.

## Current source result

- Product publishes `ProductCatalogSchemaWritePort` for every Product schema write used by mounted Commerce GraphQL: attribute and option creation, category/schema/group creation, category schema mode, schema/category bindings, Product attribute-value save, and detached-value clear.
- Mounted Commerce GraphQL no longer constructs `ProductCatalogSchemaService` for those writes. Every schema-write path resolves the host-selected `ProductCatalogCommandRuntime`, obtains its optional `schema_write_port()`, and fails closed with bounded `PRODUCT_TEMPORARILY_UNAVAILABLE` when an external profile has not supplied the capability.
- Schema writes derive tenant/actor/claims/request context from `PortContext`, retain the existing two-second Product command deadline, and scope-hash the caller key with tenant, authenticated actor, operation, and Product id when one exists.
- All eleven mounted Product schema-write resolver arguments use `String`, so the generated GraphQL SDL requires `idempotencyKey: String!` before resolver execution.
- The active Product Admin transport sends `$idempotencyKey: String!` for all eleven schema mutations and retains one caller key across an explicit retry of the same failed operation + intent; success releases the key for a later equal user action.
- Product owner schema-write failures continue through stable `PortError` mapping and bounded Commerce public codes rather than exposing owner database/validation details.

## Durable schema-create receipts

All six Product schema create operations now use the shared `rustok-outbox::idempotency` owner-operation ledger under `owner_slug = product`:

1. attribute create;
2. attribute-option create;
3. catalog-category create;
4. attribute-schema create;
5. schema-group create;
6. category-group create.

For each admitted create, the receipt binds tenant, Product owner namespace, caller idempotency key, operation, authenticated actor, and canonical request input. The receipt operation UUID is reused as the Product-owned resource ID while the receipt scope is active. A stale lease reclaim therefore reuses the same attribute, option, category, schema, or group identity instead of allocating a second row.

The receipt fence now records the **actual result produced inside the owner create method** rather than requiring the port adapter to predict the response before the Product transaction starts. `ProductWriteTransaction` captures an explicitly scoped lease plus an initially empty result slot. Each receipted create records its concrete result after its schema rows and transactional outbox publication are prepared and before commit. Transaction commit fails closed if a receipt scope has no recorded result.

This is material for `create_category`: `CatalogCategoryRecord.path` depends on the parent row read inside the Product transaction. The port does not perform an external parent/path preflight. The actual path computed by the same transaction is stored in the receipt, so lost-response replay returns the exact committed category result.

`ProductWriteTransaction` calls `idempotency::complete` with that recorded result inside the same database transaction as Product schema rows and transactional outbox publication, then commits. A response lost after commit is therefore replayed without rerunning the create or publishing a duplicate event.

A completed receipt decodes the typed create result. Reusing the same key for a different operation, actor, or request is rejected by the shared immutable request binding and mapped to bounded Product idempotency errors. A non-retryable Product failure is persisted only after the owner write future has returned and its transaction has rolled back. Retryable database/receipt failures are not converted into terminal failure receipts; the processing lease remains reclaimable under the shared stale-lease policy.

Direct Product service callers remain outside receipt semantics. Without the explicit task-local owner-port scope, create methods use normal generated resource IDs, result recording is a no-op for receipt state, and `ProductWriteTransaction` commits without receipt completion.

## Why the canonical item remains open

Durable owner replay is now source-complete for all six non-idempotent schema create operations. Update-style schema writes remain open:

- `set_category_schema_mode`;
- `bind_schema_attribute`;
- `bind_category_attribute`;
- `save_product_attribute_values`;
- `clear_detached_product_attribute_values`.

Those operations need an explicit replay result/state contract. In particular, the two attribute-value writes return projections that must replay the exact intended completed outcome rather than merely assuming repeated state mutation is harmless. The combined canonical item therefore remains `[ ]`.

Remaining source order:

1. define owner receipt/replay semantics for update-style schema writes, including exact returned attribute-value projections;
2. implement those update-style semantics through the same owner transaction fence where appropriate;
3. remove superseded Product Admin schema-write compatibility strings only after compile/source evidence confirms they are unmounted;
4. retain static/compile/parity/lost-response/restart/backend evidence as separate maintainer-run verification before any promotion.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only. No FBA/FFA status is promoted.
