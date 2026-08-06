# Checkout inventory compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

This source slice closes the Commerce consumer-side error boundary for
`InventoryReservationIdentityPort::release_inventory_by_identity` during checkout
compensation.

The retained compensation implementation remains private and unchanged. The mounted
facade wraps the constructor-injected inventory reservation port before the retained
service can use it.

## Delivered source contract

The mounted combined payment/order/inventory facade now:

- preserves the canonical inventory release request and successful response;
- preserves the owner `PortError.kind`, exact `code`, and `retryable`;
- replaces owner message text with a Commerce-owned static message before
  `CheckoutCompensationError::Boundary` construction;
- therefore prevents the owner message from reaching
  `mark_compensation_retryable` through `compensation.to_string()`;
- records only bounded context shapes, reservation-id shape, external-id
  presence/length, typed classification, message presence/length, and a redacted
  diagnostic token;
- suppresses the retained raw compatibility diagnostic only for truthful owners
  `rustok_payment`, `rustok_order`, and `rustok_inventory`;
- leaves the retained cart diagnostic active because cart cleanup is outside this
  slice.

## Static inventory messages

- validation: `Checkout inventory compensation request is invalid`
- not found: `Checkout inventory compensation resource was not found`
- conflict: `Checkout inventory compensation conflicts with the current inventory state`
- forbidden: `Checkout inventory compensation is not permitted`
- unavailable / timeout: `Checkout inventory compensation service is temporarily unavailable`
- invariant violation: `Checkout inventory compensation could not be completed safely`

## Preserved behavior

This slice does not change:

- compensation claim, lease, retry, or journal control flow;
- payment-before-order-before-inventory-before-cart ordering;
- reservation selection and status handling;
- inventory actor, locale, correlation, causation, idempotency, or deadline values
  delegated to the owner;
- release request reservation id or external identity;
- release response reservation-id, external-id, and variant-id checks;
- the durable `mark_released` checkpoint;
- consumed-reservation manual reconciliation;
- payment or order compensation behavior;
- cart snapshot/release behavior;
- FBA or FFA status.

## Remaining work

Cart snapshot and cart release consumer mapping remain open. Compile, runtime,
database, replay, restart, remote-port, workflow, and CI evidence also remain open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, inventory scenarios, database
scenarios, restart scenarios, remote-port scenarios, workflows, or CI were run.

Suggested maintainer checks:

```bash
node scripts/verify/verify-commerce-checkout-compensation-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
cargo check -p rustok-commerce --lib
```
