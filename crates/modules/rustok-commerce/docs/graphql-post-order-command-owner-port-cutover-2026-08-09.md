# Commerce GraphQL post-order command owner-port cutover

Status: `source_complete_unvalidated`

## Scope

This slice continues the canonical ecommerce standalone/topology P0 after Order published
`OrderPostOrderCommandPort` / `OrderPostOrderCommandRuntime` in #3394.

Mounted Commerce GraphQL now routes the five owner-local Order return/change writes through the
Order-owned post-order command port:

- `createStorefrontOrderReturn`;
- `createOrderChange`;
- `cancelOrderChange`;
- `createOrderReturn`;
- `cancelOrderReturn`.

The storefront path preserves the existing channel gate and `ensure_storefront_order_access`
ownership check before admitting the write. Admin paths preserve `ORDERS_UPDATE` admission and
current tenant scope.

## Runtime composition

`apps/server` now composes `OrderPostOrderCommandRuntime` into the shared host runtime, preserving an
externally supplied runtime when present and otherwise using the Order-owned in-process adapter.

`CommerceGraphqlRuntimeData` requires the host-selected runtime for mounted schemas. Only directly
embedded compatibility schemas without mounted runtime data may construct the explicit
Order-owned in-process fallback.

## Request and error boundary

The GraphQL boundary derives tenant, authenticated actor, locale, and channel from trusted request
context, applies a two-second owner-call deadline, and supplies a request-scoped idempotency key.
This slice does **not** claim durable create replay receipts; that remains separate Order-owned work.

Owner `PortError` values are mapped to the existing bounded GraphQL Order families:

- `ORDER_REQUEST_INVALID`;
- `ORDER_RESOURCE_NOT_FOUND`;
- `ORDER_STATE_CONFLICT`;
- `ORDER_TEMPORARILY_UNAVAILABLE`;
- `ORDER_OPERATION_FAILED`.

Diagnostics record bounded owner/error facts and do not expose raw owner messages or backend/core
causes.

## Deliberately unchanged

Cross-domain post-order flows remain Commerce orchestration:

- `applyOrderChange`;
- `createOrderReturnDecision`;
- `completeOrderReturn`.

They coordinate payment/refund effects and are not owner-local Order writes.

The broad implementation-plan item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and
Fulfillment concrete services behind host-composed owner ports` remains open. This cutover does not
claim all remaining concrete-service construction is gone; storefront access/read helpers and other
separately scoped topology work still require follow-up.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted GraphQL
scenarios, workflows, CI reruns, database scenarios, restart scenarios, or remote-adapter scenarios
were executed for this slice. The accompanying verifier is source-only evidence for maintainer
execution.
