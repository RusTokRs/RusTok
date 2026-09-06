# Product GraphQL lifecycle idempotency recheck — 2026-08-08

## Scope

This source-only continuation rechecks mounted Product lifecycle caller identity after Product Admin FFA PRs #3263 and #3269 plus Commerce hardening PR #3272. The canonical ecommerce source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`; this packet does not promote FBA/FFA or verification state.

## Recheck result

- Product Admin FFA retains one lifecycle caller idempotency key across a failed explicit retry and sends it as a non-null `String!` GraphQL variable on the active create/update/status/delete transport.
- Mounted Commerce GraphQL no longer manufactures a one-request `compatibility-*` identity for Product lifecycle commands.
- Mounted `createProduct`, `updateProduct`, `publishProduct`, and `deleteProduct` now expose non-null caller `idempotencyKey` arguments because the resolver inputs are `String`, not `Option<String>`.
- Empty caller keys are still rejected with `BAD_USER_INPUT`; non-empty keys remain capped at 191 bytes and scope-hashed with tenant, authenticated actor, lifecycle operation, and Product id when one exists before they become `PortContext.idempotency_key` and correlation identity.
- Foreign-actor regression callers now supply explicit lifecycle keys, so GraphQL required-argument validation does not replace the tenant/actor admission assertions those fixtures are meant to retain.
- The unknown-shipping-profile Product create fixture also supplies an explicit key, preserving the shipping-profile validation assertion under the non-null SDL.

## Canonical source status

The ecommerce plan item for Product Admin retry identity plus mandatory mounted GraphQL `idempotencyKey` is now source-complete and can be marked `[x]`. This is a source completion only: the Product lifecycle static guard, compile checks, mounted parity, retry/restart, remote-profile, and runtime evidence remain unexecuted.

The old Product Admin `transport/graphql_adapter.rs` lifecycle query strings remain unmounted compatibility source. The active Product Admin transport is `catalog_transport_retry.rs` plus `transport/product_lifecycle_graphql.rs`; cleanup of the superseded adapter should follow compile/source evidence rather than be mixed into this server-contract slice.

## Remaining Product execution order

1. Cut remaining Product schema writes away from direct `ProductCatalogSchemaService` construction through typed Product owner write capabilities with explicit write idempotency semantics.
2. Remove superseded private Product compatibility helpers only after compile/source evidence confirms no remaining consumers.
3. Execute the plan-listed static, compile, parity, remote-profile, restart, and backend evidence before any FBA/FFA promotion.

The broad ecommerce typed-owner-boundary invariant remains open because Product schema writes still construct `ProductCatalogSchemaService` directly and other ecommerce production-path debt remains separately tracked by the canonical plan.

## Verification state

No tests, checks, formatters, workflows, or runtime verification were executed in this slice per maintainer instruction. Source and GitHub diff inspection only.
