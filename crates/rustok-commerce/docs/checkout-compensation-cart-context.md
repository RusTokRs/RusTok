# Checkout compensation cart owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for the mounted
checkout compensation service's cart snapshot read and cart release calls in
`crates/rustok-commerce/src/services/checkout_compensation_owner_ports.rs`.

The mounted service already used `CartCheckoutPort`. Before this slice, both cart
calls constructed a complete `PortContext` inline and then mapped a failed
`PortError` through the generic commerce boundary mapper with only the stage name.
Correlation, actor, locale, causation, idempotency, deadline, and truthful
owner-operation attribution were therefore unavailable at the mapper.

This slice is deliberately limited to:

- cart snapshot read through `read_cart_checkout_snapshot`;
- cart release through `release_cart_checkout`.

Payment, order, and inventory compensation context retention were delivered in
preceding slices and remain unchanged.

## Delivered source contract

The mounted service now retains two cart contexts:

- `cart_read_context` is cloned into `CartCheckoutPort::read_cart_checkout_snapshot`
  and retained for failure mapping;
- `cart_release_context` is created only for a checking-out cart, cloned into
  `CartCheckoutPort::release_cart_checkout`, and retained for failure mapping.

Both failures are attributed to truthful owner `rustok_cart` with exact owner
operations:

- `read_cart_checkout_snapshot` and commerce stage `read_cart`;
- `release_cart_checkout` and commerce stage `release_cart`.

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
- payment, order, or inventory compensation callsites and context contracts;
- the cart read context actor, locale, correlation identity, causation id, or
  deadline;
- the absence of an idempotency key on the cart snapshot read;
- the cart release context actor, locale, correlation identity, causation id,
  deadline, or existing release idempotency key;
- cart snapshot request identity or locale request;
- release request identity;
- checking-out release admission;
- active-cart no-op behavior;
- completed-cart manual reconciliation;
- abandoned-cart conflict behavior;
- unknown lifecycle manual reconciliation;
- post-release active-state validation;
- `CheckoutCompensationError::Boundary` stage, code, message, or retryability;
- FBA or FFA status.

Cart errors do not use the payment/order manual-reconciliation code branch. After
diagnostics they continue through the generic `Boundary` envelope with stage
`read_cart` or `release_cart`.

## Static evidence

`scripts/verify/verify-commerce-checkout-compensation-cart-context.mjs` guards:

- one retained read context, one read delegation clone, and one mapper input;
- one retained release context, one release delegation clone, and one mapper
  input;
- truthful cart owner and exact read/release owner operations;
- existing read/write context construction and release idempotency semantics;
- full available `PortContext` fields in diagnostics;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before unchanged owner routing and boundary mapping;
- unchanged boundary envelope fields;
- unchanged cart lifecycle behavior and compensation ordering;
- unchanged payment, order, and inventory mounts;
- absence of the old inline contexts and context-dropping cart mapper calls.

The preceding payment/order and inventory verifiers were synchronized only to
stop pinning the now-superseded direct cart mapper. Their own boundary assertions
remain intact.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- remaining payment execution and compensation consumers;
- remaining order and fulfillment consumers;
- inventory, customer, tax, promotion, and remaining ecommerce adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-compensation-cart-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
