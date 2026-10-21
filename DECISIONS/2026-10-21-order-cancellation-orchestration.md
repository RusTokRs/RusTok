# Order cancellation is a commerce-owned idempotent saga over fulfillment and payment

- Date: 2026-10-21
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-commerce (orchestration); rustok-order, rustok-fulfillment, rustok-payment (owners of their state)
- Extends: [Checkout operation invariants are owned by typed Rust](./2026-10-07-checkout-operation-invariants-owned-by-rust.md)
- Supersedes: None
- Superseded by: None

## Context

Deep e-commerce review finding ECOM-POST-02: `OrderService::cancel_order` is an order-owned status
change that publishes `OrderCancelled`, and nothing consumes the event. Open fulfillments stayed
open and a captured payment was neither voided nor refunded. Reserved stock is already released:
the `order_inventory_release_on_cancel` / `order_inventory_reservation_guard` status trigger
(`m20260713_000005`) releases the order's active reservations in the same transaction as the status
write, so the orchestration does not duplicate it (see ECOM-INV-01 in the shipment ADR for the
trigger's own status).

## Decision

Admin cancellation goes through `OrderCancellationOrchestrator` in `rustok-commerce`, installed as a
decorator of `OrderAdminCommandPort` (`OrchestratedOrderAdminCommandPort`). Handlers, GraphQL and
host runtimes keep calling `order_admin_command_port().cancel`; only the runtime construction is
wrapped. The orchestrator is a **stateless, idempotent saga that can be re-run**:

1. read the order; `cancelled` returns as is; statuses outside `pending|confirmed|paid|shipped`
   are a conflict;
2. plan without side effects: every live fulfillment must be cancellable (a fulfillment that
   shipped or delivered any quantity blocks the cancellation with
   `order_cancellation.shipped_items`: the goods left the warehouse and need a return); every
   payment collection of the order is classified (`pending|authorized` → void,
   `captured` → refund, unknown status → conflict);
3. execute: cancel fulfillments through the fulfillment facade, void collections through
   `PaymentOrchestrationService::cancel_collection`, refund captured collections through
   `create_refund_idempotent` with the stable key `order_cancellation:{collection_id}:refund`,
   and only then call `OrderService::cancel_order`.

Policy chosen by the operator for captured payments: **automatic full refund** of what is captured
and not already claimed by another non-cancelled refund. The refund is created `pending` and is
completed by the provider or operator exactly like a return refund.

## Sources of truth and ownership

- Order status: `rustok-order`. Fulfillment state: `rustok-fulfillment`. Payment/refund state:
  `rustok-payment`. Stock reservations: `rustok-inventory` data guarded by the order status trigger.
- `rustok-commerce` owns only the orchestration order and the plan; it persists nothing new.
- Dependency direction: commerce → order/fulfillment/payment services already used by the other
  commerce orchestrators.

## Invariants

### Allowed states

An order becomes `cancelled` only after all its live fulfillments are cancelled and all its payment
collections are voided or fully refunded/refund-requested.

### Forbidden states

- `cancelled` order with a live fulfillment, an authorized collection, or captured money without a
  refund request.
- Cancelling an order whose goods have shipped.
- A second refund for the same collection because the command was repeated.

## Non-goals

Return/RMA of shipped goods (ECOM-POST-03), restocking shipped goods, a durable saga journal, and
a new `cancelling` order status.

## Data, transaction, and concurrency boundary

Each owner effect is its own transaction; there is no distributed transaction. The order status is
the last write, so any earlier failure leaves the order in its previous status and the command can
be repeated. Refund creation is keyed by the collection; fulfillment/payment cancellation treats an
already cancelled entity as success. A concurrent cancellation that wins the final write is
detected (`InvalidTransition` from `cancelled`) and the order is re-read.

## Context dimensions

Tenant and actor come from `PortContext`; the command requires write semantics (idempotency key and
deadline). Currency is the collection's currency; the refund amount is computed from captured and
refunded amounts of the collection.

## Events and projections

Existing owner events (`OrderCancelled`, fulfillment and payment events) are emitted by the owners.
No new event or projection.

## Failure semantics

Validation conflicts are returned before any effect. Effect failures are `unavailable` (retryable);
repeating the command resumes. A refund that the provider rejected is re-submitted with the same key.

## Migration and cutover

No schema change. Runtimes that construct `OrderAdminCommandRuntime` in commerce are wrapped;
`apps/server` needs no change because the HTTP and GraphQL runtime assembly wraps whatever
`OrderAdminCommandRuntime` the host injected.

## Alternatives considered

- Event subscriber on `order.cancelled`: asynchronous, cannot reject, and cancellation would
  already be committed with money still held.
- Guard refusing cancellation while money is committed: forces operators into a manual
  multi-step flow.
- A saga journal table: stronger audit but needs a migration that cannot be validated in this
  change; the stateless re-run saga is sufficient while every step is idempotent.
- Calling an inventory port to release stock: duplicates the status trigger.

## Verification

Unit tests in `order_cancellation.rs` (planning of fulfillments and payments, refund amount and
marker handling) and `tests/order_cancellation_test.rs` (plain order, void, refund-once, write
semantics). **Not compiled or run when written**: the authoring environment had no Rust toolchain.

## Consequences

Cancellation is no longer a single owner call; `OrderCancelled` consumers must not repeat
compensation. There is no durable per-cancellation journal; operators trace it through the owner
audit trails. Refund completion remains a provider/operator step.
