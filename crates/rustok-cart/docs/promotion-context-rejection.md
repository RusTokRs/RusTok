# Cart promotion context rejection diagnostics

Status: **source-ready / unvalidated**

## Scope

This source slice closes the remaining structured-context gap for canonical
`CartPromotionPort` preview/apply call-context rejection in
`crates/rustok-cart/src/promotion_guard.rs`.

The earlier promotion error-safety work already retained owner, correlation id,
tenant, channel, owner operation, internal cause, and stable public envelopes.
This slice extends only the read-policy and write-semantics rejection path with
the complete available `PortContext`, typed severity, and an explicit boundary.

## Delivered source contract

Both canonical owner operations remain unchanged:

- `read_cart_promotion_preview` admits `PortCallPolicy::read()`;
- `apply_cart_promotion` requires write semantics.

Both rejected admissions continue through `cart_promotion_context_error`. Before
that mapper creates the existing sanitized public `PortError`, it now emits one
structured event containing:

- owner `rustok_cart.promotion`;
- boundary `cart_promotion_context`;
- exact preview or apply owner operation;
- original `PortError` plus internal code, message, kind, and retryability;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline.

Unavailable, timeout, and invariant failures use error severity. Ordinary
validation and policy rejection use warning severity.

## Preserved behavior

This slice does not change:

- preview or apply request DTOs;
- promotion scope/kind routing;
- cart service calls or promotion calculation;
- target validation and tenant parsing;
- tax-boundary propagation;
- timeout and validation code preservation;
- fallback `cart.promotion_context_invalid` code;
- the static public message `cart promotion request context is invalid`;
- cart not-found, line-item not-found, conflict, storage-unavailable, or tax
  recalculation public envelopes;
- the legacy compatibility provider in `crates/rustok-cart/src/ports.rs`;
- FBA or FFA status.

## Static evidence

`scripts/verify/verify-cart-promotion-port-error-safety.mjs` now guards:

- exactly two preview/apply context mapper callsites;
- diagnostics before public sanitization;
- complete available `PortContext` fields;
- typed error severity and explicit boundary identity;
- unchanged timeout, validation, and fallback public mappings;
- existing target-validation, tenant-parser, owner-service, constructor-cutover,
  and compatibility constraints.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- promotion owner-service and remaining consumer/transport adapters beyond this
  admission-rejection slice;
- mounted checkout compensation inventory/cart and payment/order context retention;
- remaining order, payment, fulfillment, inventory, customer, and tax consumers;
- non-`PortError` public envelopes;
- runtime, remote-port, and cross-transport evidence.

No architecture status is promoted by source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-cart-promotion-port-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --all-features
```
