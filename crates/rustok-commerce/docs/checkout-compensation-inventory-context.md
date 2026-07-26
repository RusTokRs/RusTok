# Checkout compensation inventory owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for mounted
checkout compensation inventory reservation release in
`crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs`.

The mounted service already used `InventoryReservationIdentityPort`. Before this
slice, each reserved row constructed its complete `PortContext` inline inside
`release_inventory_by_identity`, then mapped a failed `PortError` through the
generic commerce boundary mapper with only the stage name. Correlation, actor,
locale, causation, idempotency, deadline, and truthful owner-operation
attribution were therefore unavailable at the mapper.

This slice is deliberately limited to pre-adoption inventory reservations still
in the `reserved` state. Cart snapshot read and cart release remain a separate
follow-up slice.

## Delivered source contract

For every reserved compensation row, the mounted service now:

- constructs one retained `inventory_context`;
- clones that context into `InventoryReservationIdentityPort`;
- keeps the original context available when the owner call fails;
- attributes the call to owner `rustok_inventory`;
- attributes the exact owner operation `release_inventory_by_identity`;
- preserves commerce stage `release_inventory`.

The existing shared compensation diagnostic mapper records before public error
mapping:

- the original `PortError`;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- owner, exact owner operation, and commerce stage;
- original code, public-safe message, typed kind, and retryability;
- boundary `checkout_compensation_owner_port`.

Unavailable, timeout, and invariant failures use error severity. Other owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- compensation claim, lease, retry, or journal behavior;
- payment-before-order-before-inventory-before-cart ordering;
- inventory reservation selection or status handling;
- the release request reservation id or external identity;
- the existing inventory context actor, locale, correlation identity,
  causation id, idempotency key, or deadline;
- released reservation id, external id, and variant id validation;
- the durable `mark_released` checkpoint;
- consumed-reservation manual reconciliation;
- unsupported-state conflict behavior;
- `CheckoutCompensationError::Boundary` stage, code, message, or retryability;
- payment and order compensation context behavior delivered by PR #2265;
- cart snapshot read or release mapper paths;
- FBA or FFA status.

Inventory errors do not use the payment/order manual-reconciliation code branch.
After diagnostics they continue through the generic `Boundary` envelope with
stage `release_inventory`.

## Static evidence

`scripts/verify/verify-commerce-checkout-compensation-inventory-context.mjs`
guards:

- one retained inventory context, one delegation clone, and one mapper input;
- truthful inventory owner and exact owner operation;
- the existing service actor, locale, correlation, causation, idempotency, and
  deadline construction;
- full available `PortContext` fields in diagnostics;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before unchanged owner routing and boundary mapping;
- unchanged boundary envelope fields;
- unchanged release response validation and journal checkpoint;
- unchanged compensation ordering;
- unchanged cart mapper paths;
- absence of the old inline context construction and context-dropping inventory
  mapper.

The preceding payment/order verifier was synchronized only to stop pinning the
now-superseded direct inventory mapper. Its payment/order assertions and cart
out-of-scope assertions remain intact.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- mounted compensation cart snapshot read context retention;
- mounted compensation cart release context retention;
- remaining payment execution and compensation consumers;
- remaining order, fulfillment, inventory, customer, tax, promotion, and
  ecommerce adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-compensation-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
