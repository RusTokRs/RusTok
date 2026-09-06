# Checkout cart compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

This source slice closes the Commerce consumer-side error boundary for both cart
calls made during checkout compensation:

- `CartCheckoutPort::read_cart_checkout_snapshot`;
- `CartCheckoutPort::release_cart_checkout`.

The mounted `checkout_compensation_error_safe.rs` facade retains
`checkout_compensation_owner_ports.rs` unchanged as private business logic. It
wraps the constructor-injected canonical cart port before the retained service can
use it.

## Delivered source contract

The mounted facade:

- delegates the original `PortContext`, snapshot request, release request, and
  successful `CartResponse` unchanged;
- preserves `PortError.kind`, the exact owner `code`, and `retryable`;
- replaces owner message text with a Commerce-owned static message before
  `CheckoutCompensationError::Boundary` construction;
- therefore prevents owner message text from entering
  `mark_compensation_retryable` through `compensation.to_string()`;
- records only bounded context shapes, cart-id shape, snapshot locale
  presence/length, typed classification, message presence/length, and a redacted
  diagnostic token;
- suppresses the retained raw compatibility diagnostic for truthful owners
  `rustok_payment`, `rustok_order`, `rustok_inventory`, and `rustok_cart`;
- preserves payment, order, inventory, and cart compensation ordering and
  lifecycle behavior.

## Static cart messages

- validation: `Checkout cart compensation request is invalid`
- not found: `Checkout cart compensation resource was not found`
- conflict: `Checkout cart compensation conflicts with the current cart state`
- forbidden: `Checkout cart compensation is not permitted`
- unavailable / timeout: `Checkout cart compensation service is temporarily unavailable`
- invariant violation: `Checkout cart compensation could not be completed safely`

## Preserved behavior

This slice does not change:

- compensation claim, lease, retry, or journal control flow;
- payment-before-order-before-inventory-before-cart ordering;
- cart read context construction or the absence of a read idempotency key;
- cart release context construction or its existing idempotency key;
- snapshot request `cart_id` or `locale`;
- release request `cart_id`;
- checking-out release admission;
- active-cart no-op behavior;
- completed-cart manual reconciliation;
- abandoned-cart conflict behavior;
- unknown lifecycle manual reconciliation;
- post-release active-state validation;
- payment, order, or inventory compensation behavior;
- FBA or FFA status.

## Remaining work

The checkout compensation owner-port consumer mapper is source-closed for payment,
order, inventory, and cart. The broader ecommerce mapper task remains open for
other services and non-`PortError` public envelopes. Compile, runtime, database,
replay, restart, remote-port, workflow, and CI evidence also remain open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, cart scenarios, database
scenarios, restart scenarios, remote-port scenarios, workflows, or CI were run.

Suggested maintainer checks:

```bash
node scripts/verify/verify-commerce-checkout-compensation-cart-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
cargo check -p rustok-commerce --lib
```
