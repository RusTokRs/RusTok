# Typed ecommerce lifecycle status foundation

Status: `owner_views_and_order_consumers_source_ready_unvalidated`

This source wave adds backward-compatible typed lifecycle views for the four owners
used by the mounted checkout path. Persisted columns and existing transport fields
remain strings until migrations and consumer parity are proven.

## Owner views

### Cart

- Reuse the canonical owner `CartStatus`: `Active`, `CheckingOut`, `Completed`,
  `Abandoned`.
- `CartResponse::lifecycle_status()` parses the raw persisted value and fails closed
  for unknown legacy or external values.
- predicates for checkout admission, completion, and terminal state live on the
  canonical enum.
- a second cart status enum or `crates/rustok-cart/src/status.rs` is forbidden.

### Order

- `OrderStatusKind`: `Pending`, `Confirmed`, `Paid`, `Shipped`, `Delivered`,
  `Cancelled`, `Unknown`.
- `OrderChangeStatusKind`: `Pending`, `Applied`, `Cancelled`, `Unknown`.
- `OrderReturnStatusKind`: `Pending`, `Completed`, `Cancelled`, `Unknown`.
- typed accessors on order, order-change, and return responses.
- conservative transition and financial-effect predicates.
- checkout payment settlement dispatches through `OrderStatusKind`.
- checkout compensation dispatches through `OrderStatusKind`; unknown states require
  manual reconciliation rather than automatic cancellation.

### Payment

- `PaymentCollectionStatusKind`: `Pending`, `Authorized`, `Captured`, `Cancelled`,
  `Unknown`.
- `PaymentStatusKind`: `Pending`, `Authorized`, `Captured`, `Cancelled`, `Unknown`.
- `RefundStatusKind`: `Pending`, `Refunded`, `Cancelled`, `Unknown`.
- typed accessors on collection, payment, and refund responses.
- authorize/capture/terminal predicates for collection consumers.

### Fulfillment

- `FulfillmentStatusKind`: `Pending`, `Shipped`, `Delivered`, `Cancelled`, `Unknown`.
- `FulfillmentResponse::status_kind()`.
- conservative ship, deliver, and terminal predicates.

## Compatibility rule

Unknown legacy or external status strings are preserved in the existing raw `status`
field. Order, payment, and fulfillment map them to `Unknown`; cart returns an explicit
owner error from `lifecycle_status()`. The owner layer must not guess a lifecycle state
or rewrite historical values merely to satisfy a typed consumer.

## Still open

- Replace critical string comparisons in payment execution, checkout payment stage,
  fulfillment stage, payment compensation, cart checkout, and recovery.
- Move persistence transitions to typed CAS commands and database constraints.
- Add typed filter inputs after REST, GraphQL, native, and admin compatibility is
  prepared.
- Remove string status transport fields only in a versioned breaking contract.
- Execute compile, migration, lifecycle, replay, and transport parity evidence.

## Verification

- `node scripts/verify/verify-payment-typed-lifecycle-statuses.mjs`
- `node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`
- `node scripts/verify/verify-order-payment-settlement-typed-status.mjs`
- `node scripts/verify/verify-order-compensation-typed-status.mjs`
- targeted cart/order/payment/fulfillment unit tests

No verification command was executed in this source wave.
