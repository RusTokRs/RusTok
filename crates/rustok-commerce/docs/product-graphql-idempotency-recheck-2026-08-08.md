# Product GraphQL lifecycle idempotency recheck — 2026-08-08

## Scope

This source-only continuation rechecks mounted Product lifecycle caller identity after Product Admin FFA PRs #3263 and #3269. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; this packet does not promote FBA/FFA or verification state.

## Recheck result

- Product Admin FFA now retains one lifecycle caller idempotency key across a failed explicit retry and sends it as a non-null `String!` GraphQL variable on the active create/update/status/delete transport.
- Mounted Commerce GraphQL no longer manufactures a one-request `compatibility-*` identity when a lifecycle caller omits `idempotencyKey`.
- `product_command_context` now rejects omission with public `BAD_USER_INPUT` / `Product mutation idempotency key is required` after existing module, permission, tenant-actor, and create/update shipping-profile admission has run.
- Non-empty caller keys remain capped at 191 bytes and scope-hashed with tenant, authenticated actor, lifecycle operation, and Product id when one exists before they become `PortContext.idempotency_key` and correlation identity.
- The lifecycle resolver arguments intentionally remain `Option<String>` in this slice. This preserves the current ordering of foreign-actor and shipping-profile regression fixtures while removing the unsafe compatibility execution path.

## Why the canonical item remains open

The ecommerce plan item that says Product Admin FFA must retain one key and mounted GraphQL `idempotencyKey` must become mandatory remains `[ ]` because the GraphQL SDL is still nullable. The remaining source step is narrow and explicit:

1. update the foreign-actor and unknown-shipping-profile GraphQL fixtures to supply caller idempotency keys while preserving the admission assertions they actually test;
2. change `createProduct`, `updateProduct`, `publishProduct`, and `deleteProduct` resolver arguments from `Option<String>` to `String`, producing non-null GraphQL arguments;
3. update both Product lifecycle source guards to forbid nullable lifecycle idempotency arguments;
4. only then mark the canonical FFA/server-idempotency item complete.

The old Product Admin `transport/graphql_adapter.rs` lifecycle query strings remain unmounted compatibility source. The active Product Admin transport is `catalog_transport_retry.rs` plus `transport/product_lifecycle_graphql.rs`; cleanup of the superseded adapter should follow compile/source evidence rather than be mixed into this server hardening slice.

## Remaining Product execution order

1. Complete the non-null GraphQL SDL cutover and regression-fixture caller update described above.
2. Cut remaining Product schema writes away from direct `ProductCatalogSchemaService` construction through typed Product owner write capabilities with explicit write idempotency semantics.
3. Remove superseded private Product compatibility helpers only after compile/source evidence confirms no remaining consumers.
4. Execute the plan-listed static, compile, parity, remote-profile, restart, and backend evidence before any FBA/FFA promotion.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only.
