# Commerce GraphQL order lifecycle owner-port cutover

Status: `source_complete_unvalidated`

## Scope

This slice continues the canonical ecommerce topology P0 by removing mounted Commerce GraphQL construction of the Order concrete service from the four order lifecycle mutations that already have a published Order owner command capability.

Mounted fields covered by this slice:

- `markOrderPaid`
- `shipOrder`
- `deliverOrder`
- `cancelOrder`

The fields keep their current tenant and `ORDERS_UPDATE` admission, explicit authenticated-actor anti-spoof check, inputs, and response projection.

## Owner boundary

Mounted schema data now requires the host-composed `rustok_order::OrderAdminCommandRuntime`. The server already composes that runtime into `HostRuntimeContext`, preferring an externally supplied runtime and otherwise creating the Order-owned in-process adapter.

The four mounted GraphQL mutations call `OrderAdminCommandPort` with a request-owned `PortContext` carrying:

- tenant identity;
- authenticated user actor identity;
- resolved request locale;
- resolved request channel when present;
- a bounded two-second deadline;
- a deterministic mutation-scoped idempotency identity;
- a correlation identity scoped to the order and lifecycle operation.

Owner `PortError` values are mapped to stable GraphQL Order error families with bounded diagnostics. Raw owner messages and technical database/core details are not projected through the new mapper.

Directly embedded compatibility schemas that do not carry `CommerceGraphqlRuntimeData` retain the explicit Order-owned in-process runtime fallback. Mounted schemas use the host-selected runtime.

## Deliberately still open

The broad canonical topology item remains open. `CommerceFulfillmentMutation` still has concrete Order service construction for post-order return/change commands such as storefront/admin return creation, order-change creation/cancellation, and return cancellation. Those operations do not yet have the same published owner command contract and should be addressed as a separate Order-owner capability slice rather than folded into this lifecycle cutover.

The checkout compatibility source, Product schema-write replay work, Product dependency contract, mapper cleanup, remote/mounted evidence, and other canonical open items are unchanged.

## Validation state

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted GraphQL scenarios, workflows, CI reruns, runtime calls, database scenarios, restart scenarios, or remote-adapter scenarios were executed for this slice.

A source-only verifier is added for later maintainer execution:

```bash
node scripts/verify/verify-commerce-graphql-order-lifecycle-owner-port-cutover.mjs
```
