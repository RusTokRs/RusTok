# Product schema write boundary recheck — 2026-08-08

## Scope

This source-only continuation follows Product schema-write capability publication and rechecks the mounted Commerce GraphQL consumers plus the active Product Admin callers. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; its combined Product schema-write/idempotency item remains open in this slice.

## Current source result

- Product publishes `ProductCatalogSchemaWritePort` for every Product schema write used by mounted Commerce GraphQL: attribute and option creation, category/schema/group creation, category schema mode, schema/category bindings, Product attribute-value save, and detached-value clear.
- Mounted Commerce GraphQL no longer constructs `ProductCatalogSchemaService` for those writes. Every successful schema-write path resolves the host-selected `ProductCatalogCommandRuntime`, obtains its optional `schema_write_port()`, and fails closed with bounded `PRODUCT_TEMPORARILY_UNAVAILABLE` when an external profile has not supplied the capability.
- Schema writes derive tenant/actor/claims/request context from `PortContext`, retain the existing two-second Product command deadline, and scope-hash the caller key with tenant, authenticated actor, operation, and Product id when one exists.
- The active Product Admin transport no longer re-exports schema-write calls from the legacy GraphQL adapter. It sends `$idempotencyKey: String!` for all eleven schema mutations and retains one caller key across an explicit retry of the same failed operation + intent; success releases the key for a later equal user action.
- Product owner schema-write failures continue through stable `PortError` mapping and bounded Commerce public codes rather than exposing owner database/validation details.

## Why the GraphQL SDL is still nullable

The mounted schema-write resolver inputs intentionally remain `Option<String>` in this slice. Omission is rejected with `BAD_USER_INPUT` / `Product schema mutation idempotency key is required` after existing module, permission, tenant-actor, and mutation-input admission has run.

This preserves the current `admin_graphql_rejects_foreign_actor_for_every_product_mutation` fixture, which intentionally omits schema-write keys while asserting that every Product mutation reaches tenant/actor admission. Making these arguments non-null immediately would move the failure to GraphQL required-argument validation and erase the regression signal. Active Product Admin callers already send non-null keys, so successful mounted schema-write execution has no compatibility-generated identity.

The next narrow SDL step is therefore to add explicit keys to those foreign-actor schema mutation callers and then change all eleven resolver arguments from `Option<String>` to `String`.

## Why the canonical item remains open

The embedded `ProductCatalogSchemaService` still does not persist a command receipt keyed by `PortContext.idempotency_key`. Caller identity is now explicit and retained by Product Admin across failed explicit retries, but durable owner replay/adoption semantics must not be inferred from that boundary contract. A response lost after owner commit can still make a create retry unsafe until Product persists/replays the completed outcome.

Remaining source order:

1. update the foreign-actor regression document with explicit schema-write caller keys and make mounted schema-write `idempotencyKey` non-null;
2. implement the owner-local durable receipt/replay contract needed for non-idempotent schema creates and define the outcome for update-style writes;
3. remove superseded Product Admin schema-write compatibility strings only after compile/source evidence confirms they are unmounted;
4. update canonical completion only when mounted non-null caller identity and intended owner replay semantics are source-complete.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only. No FBA/FFA status is promoted.
