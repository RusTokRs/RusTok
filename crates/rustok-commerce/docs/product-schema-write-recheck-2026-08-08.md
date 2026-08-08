# Product schema write boundary recheck — 2026-08-08

## Scope

This source-only continuation follows Product schema-write capability publication, mounted consumer cutover, mandatory GraphQL caller identity, durable receipts for all six schema create operations, and durable receipts for the three unit-result schema state updates. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`. This slice closes the remaining Product attribute-value source gap by capturing the exact completed projection inside the owner transaction before receipt completion and commit. Runtime, backend, restart, and lost-response evidence remain maintainer-run work and are not promoted here.

## Current source result

- Product publishes `ProductCatalogSchemaWritePort` for all eleven mounted Product schema writes used by Commerce GraphQL: attribute and option creation, category/schema/group creation, category schema mode, schema/category bindings, Product attribute-value save, and detached-value clear.
- Mounted Commerce GraphQL no longer constructs `ProductCatalogSchemaService` for those writes. Every schema-write path resolves the host-selected `ProductCatalogCommandRuntime`, obtains its optional `schema_write_port()`, and fails closed with bounded `PRODUCT_TEMPORARILY_UNAVAILABLE` when an external profile has not supplied the capability.
- Schema writes derive tenant/actor/claims/request context from `PortContext`, retain the existing two-second Product command deadline, and scope-hash the caller key with tenant, authenticated actor, operation, and Product id when one exists.
- All eleven mounted Product schema-write resolver arguments use `String`, so the generated GraphQL SDL requires `idempotencyKey: String!` before resolver execution.
- The active Product Admin transport sends `$idempotencyKey: String!` for all eleven schema mutations and retains one caller key across an explicit retry of the same failed operation + intent; success releases the key for a later equal user action.
- Product owner schema-write failures continue through stable `PortError` mapping and bounded Commerce public codes rather than exposing owner database/validation details.

## Durable schema-create receipts

All six Product schema create operations use the shared `rustok-outbox::idempotency` owner-operation ledger under `owner_slug = product`:

1. attribute create;
2. attribute-option create;
3. catalog-category create;
4. attribute-schema create;
5. schema-group create;
6. category-group create.

For each admitted create, the receipt binds tenant, Product owner namespace, caller idempotency key, operation, authenticated actor, and canonical request input. The receipt operation UUID is reused as the Product-owned resource ID while the receipt scope is active. A stale lease reclaim therefore reuses the same attribute, option, category, schema, or group identity instead of allocating a second row.

The receipt fence records the actual result produced inside the owner method rather than requiring the port adapter to predict the response before the Product transaction starts. `ProductWriteTransaction` captures an explicitly scoped lease plus an initially empty result slot. Each receipted create records its concrete result after its schema rows and transactional outbox publication are prepared and before commit. Transaction commit fails closed if a receipt scope has no recorded result.

This remains material for `create_category`: `CatalogCategoryRecord.path` depends on the parent row read inside the Product transaction. The actual path computed by that transaction is stored in the receipt, so lost-response replay returns the exact committed category result without rerunning the create.

## Durable unit-result state-write receipts

The three update-style state writes use the same owner receipt fence:

1. `set_category_schema_mode`;
2. `bind_schema_attribute`;
3. `bind_category_attribute`.

Each owner method records successful `()` as JSON `null` after its mutation and transactional outbox publication are prepared and before `ProductWriteTransaction::commit`. Receipt completion therefore commits atomically with the state update and its domain event. Lost-response replay decodes the stored `null` back to `()` and does not rerun the upsert or publish a duplicate event.

## Exact Product attribute-value replay

The remaining two write methods now participate in the same durable owner receipt protocol:

- `save_product_attribute_values`;
- `clear_detached_product_attribute_values`.

Their response type is `Vec<ProductAttributeValueRecord>`, so a later database read is not an acceptable replay source: another legitimate Product write could change the projection after the original command commits. The response therefore has to be captured from the exact owner transaction that contains the EAV mutation and outbox publication.

`load_product_attribute_values` is now backed by a connection-neutral projection helper using SeaORM `ConnectionTrait`. The helper can read from both the normal `DatabaseConnection` and the active `ProductWriteTransaction`. Effective-form resolution uses the same connection-neutral path, including existing-value discovery, category/schema maps, localized values, option rows, ordering, and detached classification. The public read behavior therefore keeps the existing projection semantics while the write path can observe its own uncommitted mutation.

For `save_product_attribute_values`, Product applies all validated patches, publishes `ProductAttributeValuesChanged` when the patch set is non-empty, loads the exact completed projection from the active transaction, records that vector into the receipt result slot, and only then commits. A lost response is replayed from the completed receipt instead of rerunning the EAV mutation or reconstructing state later.

For `clear_detached_product_attribute_values`, the delete and event publication remain conditional on a non-empty detached target set, but projection capture and receipt completion are unconditional. The empty-target detached clear therefore opens an owner transaction, reads the unchanged exact projection inside that transaction, records it, and commits the receipt without fabricating an outbox event. This prevents a successful no-op from leaving an admitted lease permanently incomplete.

Request binding for both methods includes authenticated actor, Product id, locale, and the exact patch or attribute-id payload. Reusing one caller key with a different Product, locale, patch set, clear target, actor, or operation remains an immutable-request conflict rather than a second execution.

## Source completion boundary

The mounted Product schema-write boundary now has source-complete durable owner replay across all eleven operations:

1. six create operations with stable receipt-owned resource IDs and exact transaction-derived results;
2. three state updates with atomically stored unit results;
3. two Product attribute-value writes with exact transaction-local EAV projection results, including the no-op clear path.

This closes the implementation gap identified by the prior recheck. It does not provide runtime promotion evidence. Compile coverage, static verifier execution, lost-response injection, stale-lease reclaim, restart replay, tenant/request conflict checks, and SQLite/PostgreSQL/MySQL evidence remain separate maintainer-run verification work. Superseded Product Admin compatibility strings should still be removed only after compile/source evidence confirms they are unmounted.

## Verification state

Source and GitHub diff inspection only. No tests, checks, formatters, workflows, or runtime verification were executed per maintainer instruction. No FBA/FFA status is promoted.
