# Product schema write boundary recheck — 2026-08-08

## Scope

This source-only continuation follows Product schema-write capability publication and mounted consumer cutover. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; its combined Product schema-write/idempotency item remains open because durable owner replay semantics are still missing.

## Current source result

- Product publishes `ProductCatalogSchemaWritePort` for every Product schema write used by mounted Commerce GraphQL: attribute and option creation, category/schema/group creation, category schema mode, schema/category bindings, Product attribute-value save, and detached-value clear.
- Mounted Commerce GraphQL no longer constructs `ProductCatalogSchemaService` for those writes. Every schema-write path resolves the host-selected `ProductCatalogCommandRuntime`, obtains its optional `schema_write_port()`, and fails closed with bounded `PRODUCT_TEMPORARILY_UNAVAILABLE` when an external profile has not supplied the capability.
- Schema writes derive tenant/actor/claims/request context from `PortContext`, retain the existing two-second Product command deadline, and scope-hash the caller key with tenant, authenticated actor, operation, and Product id when one exists.
- All eleven mounted Product schema-write resolver arguments now use `String`, so the generated GraphQL SDL requires `idempotencyKey: String!` before resolver execution.
- The foreign-actor regression document supplies an explicit caller key for every Product schema mutation, preserving tenant/actor admission coverage after the SDL becomes non-null.
- The active Product Admin transport already sends `$idempotencyKey: String!` for all eleven schema mutations and retains one caller key across an explicit retry of the same failed operation + intent; success releases the key for a later equal user action.
- Product owner schema-write failures continue through stable `PortError` mapping and bounded Commerce public codes rather than exposing owner database/validation details.

## Mandatory GraphQL caller identity

There is no compatibility-generated schema-write identity and no nullable schema-write idempotency argument in mounted Commerce GraphQL. The same caller-key trim and 191-byte validation used by Product lifecycle mutations applies before the schema-write owner port is called.

The existing `admin_graphql_rejects_foreign_actor_for_every_product_mutation` fixture now provides explicit schema-write keys. That keeps the regression focused on tenant/actor admission instead of GraphQL missing-argument validation while preserving the mandatory SDL contract for every real caller.

## Why the canonical item remains open

The embedded `ProductCatalogSchemaService` still does not persist a command receipt keyed by `PortContext.idempotency_key`. Caller identity is explicit and Product Admin retains it across failed explicit retries, but durable owner replay/adoption semantics must not be inferred from that boundary contract. A response lost after owner commit can still make a non-idempotent create retry unsafe until Product persists and replays the completed outcome.

Remaining source order:

1. implement the owner-local durable receipt/replay contract needed for non-idempotent schema creates and define replay behavior for update-style schema writes;
2. remove superseded Product Admin schema-write compatibility strings only after compile/source evidence confirms they are unmounted;
3. update canonical completion only when the intended owner replay semantics are source-complete;
4. retain static/compile/parity/remote/restart/backend evidence as separate maintainer-run verification.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only. No FBA/FFA status is promoted.
