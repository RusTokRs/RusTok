# Order post-order command owner capability

Status: `source_complete_unvalidated`

## Scope

This slice continues the canonical ecommerce topology P0 after GraphQL order lifecycle cutover PR #3392.

The mounted Commerce GraphQL fulfillment/post-order mutation object still has five direct `OrderService` construction paths for owner-local post-order writes:

- storefront order-return creation;
- admin order-change creation;
- admin order-change cancellation;
- admin order-return creation;
- admin order-return cancellation.

Before cutting those consumers over, this slice publishes the missing Order-owned command boundary rather than moving foreign persistence or lifecycle policy into Commerce.

## Owner capability

`rustok-order` now publishes `OrderPostOrderCommandPort` and `OrderPostOrderCommandRuntime` with four owner-local operations:

- `create_change`;
- `cancel_change`;
- `create_return`;
- `cancel_return`.

The in-process adapter delegates to the existing `OrderService` methods and keeps Order persistence, validation, lifecycle policy, and event behavior inside `rustok-order`.

The request contracts wrap the existing Order DTOs and stable resource identities rather than duplicating transport-specific GraphQL shapes.

## Boundary and error policy

The port requires write admission through `PortContext`, validates tenant and actor identities, and maps `OrderError` into bounded `PortError` families.

Database/core details are never copied into public `PortError.message`. Diagnostics log stable operation/error variants plus correlation/resource shape without raw backend errors.

This source slice does not claim durable replay receipts for post-order create operations. The mounted consumer cutover must preserve explicit caller context and the broader ecommerce production gate remains open until replay/runtime evidence is retained.

## Explicitly not changed

This slice does not change:

- mounted GraphQL resolvers;
- Commerce post-order payment/refund orchestration;
- return completion or order-change apply flows;
- database schema or migrations;
- FBA/FFA promotion state.

The next slice can host-compose this runtime and replace the five remaining direct GraphQL `OrderService` owner-local write calls.

## Validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatting, workflows, CI reruns, database scenarios, mounted GraphQL scenarios, restart scenarios, or remote-adapter scenarios were executed for this slice.
