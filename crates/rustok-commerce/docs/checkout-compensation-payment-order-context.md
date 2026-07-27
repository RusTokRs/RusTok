# Checkout compensation payment/order owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for the mounted
checkout compensation service's payment and order owner calls in
`crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs`.

The mounted service already used typed `CheckoutPaymentCompensationPort` and
`CheckoutOrderCompensationPort` boundaries. Before this slice, both calls built a
complete `PortContext` inline and then mapped a failed `PortError` with only the
commerce stage. Correlation, actor, locale, causation, idempotency, deadline, and
truthful owner-operation attribution were therefore unavailable at the mapper.

This slice is deliberately limited to:

- payment compensation through `compensate_checkout_payment`;
- order compensation through `compensate_checkout_order`.

Inventory reservation release and cart read/release remain separate follow-up
slices.

## Delivered source contract

The mounted service now retains one context for each owner call:

- `payment_context` is cloned into `CheckoutPaymentCompensationPort` and retained
  for failure mapping;
- `order_context` is cloned into `CheckoutOrderCompensationPort` and retained for
  failure mapping.

A shared commerce diagnostic mapper receives the retained context plus:

- truthful payment owner `rustok_payment` or order owner `rustok_order`;
- exact owner operation `compensate_checkout_payment` or
  `compensate_checkout_order`;
- existing commerce stage `compensate_payment` or `compensate_order`;
- original `PortError`.

Before the existing manual-reconciliation or boundary mapping runs, the mapper
emits one structured event containing:

- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- owner, exact owner operation, and commerce stage;
- original error code, public-safe message, typed kind, and retryability;
- boundary `checkout_compensation_owner_port`.

Unavailable, timeout, and invariant failures use error severity. Other owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- checkout compensation claim, lease, retry, or journal behavior;
- captured-funds manual-reconciliation admission;
- payment-before-order-before-inventory-before-cart compensation ordering;
- payment or order request DTOs, reasons, metadata, or idempotency keys;
- typed payment collection and order cancelled-state validation;
- missing owner result reconciliation behavior;
- manual-reconciliation code recognition for payment and order owners;
- `CheckoutCompensationError::ManualReconciliation` contents;
- `CheckoutCompensationError::Boundary` stage, code, message, or retryability;
- inventory reservation release callsites or mapper;
- cart read/release callsites or mapper;
- FBA or FFA status.

## Static evidence

`scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs`
guards:

- one retained payment context and one delegation clone;
- one retained order context and one delegation clone;
- truthful owner and exact owner-operation attribution;
- full available `PortContext` fields;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before the unchanged manual-reconciliation/boundary routing;
- unchanged boundary envelope fields;
- unchanged payment/order lifecycle checks and compensation ordering;
- unchanged inventory/cart mapper paths;
- absence of the old inline context construction and context-dropping mapper calls.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- mounted compensation inventory reservation release context retention;
- mounted compensation cart read/release context retention;
- remaining payment execution and compensation consumers;
- remaining order, fulfillment, inventory, customer, tax, promotion, and ecommerce
  adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
