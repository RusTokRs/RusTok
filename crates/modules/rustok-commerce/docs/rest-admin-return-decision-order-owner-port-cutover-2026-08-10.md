# Commerce REST admin return-decision Order owner-port cutover

Status: `source_complete_unvalidated`

Date: 2026-08-10

## Scope

This slice cuts only the mounted REST
`POST /admin/orders/{id}/returns/decision` Order-owned writes away from concrete
`OrderService` construction inside Commerce post-order orchestration.

The route keeps the existing `ORDERS_UPDATE` admission and conditional
`PAYMENTS_UPDATE` admission. It now supplies the host-selected
`OrderPostOrderCommandPort` from `CommerceHttpRuntime` to
`ReturnDecisionOwnerOrchestrationService`.

The following Order-owned effects use the owner command port in the mounted REST
return-decision path:

- create the return through `create_return`;
- create an exchange or claim order change through `create_change`;
- complete the return through the new `complete_return` capability.

## Order owner capability

`OrderPostOrderCommandPort` now publishes `complete_return` with typed
`CompleteOrderReturnRequest`.

The in-process adapter delegates to the same owner-local
`OrderService::complete_return` implementation used before this cutover. The trait
method has a default fail-closed implementation so existing external adapters remain
source-compatible; an external adapter that does not implement return completion
returns `order.post_order_complete_return_unavailable` after normal write admission.

The mounted REST route therefore does not silently fall back to an embedded Order
service when a host-selected adapter lacks the capability.

## Context and replay limitation

The route builds one authenticated root `PortContext` with tenant, user actor,
request locale/channel, correlation identity, a two-second deadline, and a generated
UUID idempotency identity.

The return-decision orchestration derives a distinct correlation/idempotency identity
for each Order owner operation (`create_return`, `create_change`, and
`complete_return`). The legacy route does not expose a caller idempotency key, so the
generated identity is **write-admission metadata only**. This slice does not claim
durable replay or exactly-once semantics for repeated client requests.

## Preserved decision semantics

The pre-cutover validation and action semantics are preserved:

- input validation and action-shape validation happen before return creation;
- `return_only` creates and completes the return;
- `refund` creates the return, performs the existing Payment refund flow, then
  completes the return with the refund id;
- `exchange` and `claim` create the same return-linked order-change payloads, then
  complete the return with the order-change id;
- response action, return, refund, order-change, and metadata projection remain the
  same.

## Error boundary

Order owner `PortError` values map to the existing admin Order public families:
validation, not found, state conflict, storage unavailable, and fail-closed internal
failure. The new mapper records bounded facts only: error kind, owner code length,
retryability, correlation identity, non-nil request identities, public code/status,
and boundary. It does not log raw `PortError.message` or stringify the owner error.

Existing Payment and post-order validation errors still use the pre-existing
post-order mapper in this slice so their public envelopes are unchanged.

## Explicitly deferred topology gaps

This slice is deliberately Order-only and REST-only.

- The refund branch still uses `PaymentService` to find the captured payment
  collection and the existing Payment orchestration to create the refund. Moving
  that Payment dependency behind host-composed Payment ports is a separate mounted
  gap.
- Mounted GraphQL `createOrderReturnDecision` still uses the legacy
  `PostOrderOrchestrationService::create_return_decision` path and is a separate
  transport cutover.
- Mounted `/admin/returns/{id}/complete` uses the broader
  `ReturnCompletionOrchestrationService`; its remaining Order/Payment construction is
  a separate cross-domain slice.
- Legacy create/list/show/cancel functions in `controllers/admin/returns.rs` still
  contain concrete Order service source, but the admin router mounts their existing
  `post_order_commands` / `post_order_reads` replacements instead.

The broad topology P0 therefore remains open:

`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and Fulfillment concrete services behind host-composed owner ports.`

## Validation

No tests, Cargo commands, Node verifiers, formatter, REST/GraphQL scenarios,
workflows/CI, database scenarios, provider calls, restart scenarios, or remote-adapter
scenarios were executed in this slice. Source guards were added/updated but
intentionally not run.
