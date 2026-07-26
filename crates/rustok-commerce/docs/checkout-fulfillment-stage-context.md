# Checkout fulfillment stage owner context

Status: **source-ready / unvalidated**

## Scope

This slice retains the exact `PortContext` used by the mounted durable checkout
fulfillment stage for three typed owner calls:

- `CheckoutFulfillmentExecutionPort::ensure_checkout_fulfillments`;
- `CheckoutFulfillmentExecutionPort::read_checkout_fulfillments`;
- `CheckoutOrderPaymentSettlementPort::settle_checkout_payment`.

Each call now constructs one context value, delegates a clone of that value to
the owner port, and reuses the original context when mapping a returned
`PortError` into `CheckoutFulfillmentStageError::Boundary`.

## Retained diagnostic context

The structured boundary event records:

- truthful owner identity (`rustok_fulfillment` or `rustok_order`);
- correlation and tenant identity;
- actor, channel, and locale;
- causation, traceparent, idempotency key, and deadline;
- exact owner operation and commerce stage;
- stable owner error code, typed error kind, and retryability;
- explicit `commerce_checkout_fulfillment_stage` boundary identity.

Unavailable, timeout, and invariant failures use error severity. Validation,
not-found, conflict, forbidden, and other policy rejections use warning severity.
The complete typed `PortError` remains internal diagnostic data.

## Preserved contracts

This slice does not change:

- fulfillment or order owner port traits;
- ensure/read/settlement request and response DTOs;
- fulfillment plan construction and cart-line provenance checks;
- captured-payment admission and amount validation;
- payment-reference or manual-provider fallback behavior;
- fulfillment idempotency and order-settlement idempotency keys;
- bounded stage-loop behavior;
- the `payment_captured -> fulfillment_created` checkpoint;
- `CheckoutFulfillmentStageError::Boundary` stage, code, message, or retryability;
- FBA or FFA status.

## Static evidence

The focused source guard is:

```text
scripts/verify/verify-commerce-checkout-fulfillment-stage-context.mjs
```

It verifies three retained contexts, three cloned owner delegations, exact owner
and operation attribution, structured diagnostic fields, severity policy,
existing typed lifecycle admission, checkpoint preservation, and removal of the
old context-dropping mapper.

## Still open

This slice does not deliver:

- fulfillment or order owner policy-admission diagnostics;
- checkout compensation context retention;
- remaining fulfillment GraphQL/HTTP/native adapters;
- remaining order, payment, inventory, customer, tax, or promotion mapper cleanup;
- runtime, restart, remote-profile, or cross-database evidence.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-fulfillment-stage-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```

These commands were not run by the implementation agent.
