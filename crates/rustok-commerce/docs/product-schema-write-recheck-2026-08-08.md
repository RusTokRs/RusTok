# Product schema write boundary recheck — 2026-08-08

## Scope

This source-only continuation follows Product schema-write capability publication, mounted consumer cutover, and mandatory GraphQL caller identity. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; its combined Product schema-write/idempotency item remains open while durable owner replay is completed operation by operation.

## Current source result

- Product publishes `ProductCatalogSchemaWritePort` for every Product schema write used by mounted Commerce GraphQL: attribute and option creation, category/schema/group creation, category schema mode, schema/category bindings, Product attribute-value save, and detached-value clear.
- Mounted Commerce GraphQL no longer constructs `ProductCatalogSchemaService` for those writes. Every schema-write path resolves the host-selected `ProductCatalogCommandRuntime`, obtains its optional `schema_write_port()`, and fails closed with bounded `PRODUCT_TEMPORARILY_UNAVAILABLE` when an external profile has not supplied the capability.
- Schema writes derive tenant/actor/claims/request context from `PortContext`, retain the existing two-second Product command deadline, and scope-hash the caller key with tenant, authenticated actor, operation, and Product id when one exists.
- All eleven mounted Product schema-write resolver arguments use `String`, so the generated GraphQL SDL requires `idempotencyKey: String!` before resolver execution.
- The active Product Admin transport sends `$idempotencyKey: String!` for all eleven schema mutations and retains one caller key across an explicit retry of the same failed operation + intent; success releases the key for a later equal user action.
- Product owner schema-write failures continue through stable `PortError` mapping and bounded Commerce public codes rather than exposing owner database/validation details.

## Durable attribute-create receipts

Product attribute and attribute-option creates now use the shared `rustok-outbox::idempotency` owner-operation ledger under `owner_slug = product`.

For each admitted create, the receipt binds tenant, Product owner namespace, caller idempotency key, operation, actor, and canonical request input. The receipt operation UUID is reused as the Product-owned attribute or option ID while the receipt scope is active. This gives a reclaimed attempt the same resource identity instead of allocating a second row.

`ProductWriteTransaction` captures the explicitly scoped receipt lease. On success it calls `idempotency::complete` inside the same database transaction as the Product schema rows and transactional outbox publication, then commits that transaction. A response lost after commit is therefore replayed from the completed owner receipt without rerunning the create or publishing a duplicate event.

A completed receipt decodes the stored `ProductAttributeRecord` or `ProductAttributeOptionRecord`. Reusing the same key for a different operation or request is rejected by the shared immutable request binding and mapped to bounded Product idempotency errors. A non-retryable Product failure is persisted only after the owner write future has returned and its transaction has rolled back. Retryable database/receipt failures are not converted into terminal failure receipts; the processing lease remains reclaimable under the shared stale-lease policy.

Direct Product service callers are not silently changed into receipt clients: outside the explicitly scoped owner-port execution, `current_product_operation_id()` is absent, create methods retain normal generated IDs, and `ProductWriteTransaction` commits without receipt completion.

## Why the canonical item remains open

This is intentionally a partial durable-idempotency slice. Category, schema, schema-group, and category-group creates still need owner receipt/replay semantics. Update-style schema writes (`set_category_schema_mode`, schema/category binding, attribute-value save, and detached-value clear) still need their replay result/state contract defined and implemented.

In short, attribute and attribute-option creates are durably receipted, while category/schema/group creates and update-style schema writes remain open. The combined canonical item must stay `[ ]` until those remaining write semantics are source-complete.

Remaining source order:

1. extend the same owner-transaction receipt fence to category/schema/group creates with stable Product-owned resource identities;
2. define and implement replay semantics for update-style schema writes, including the returned attribute-value projections;
3. remove superseded Product Admin schema-write compatibility strings only after compile/source evidence confirms they are unmounted;
4. retain static/compile/parity/lost-response/restart/backend evidence as separate maintainer-run verification before any promotion.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only. No FBA/FFA status is promoted.
