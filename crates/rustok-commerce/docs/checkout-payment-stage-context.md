# Checkout payment stage owner context

Status: **source-ready / unvalidated**

## Scope

This slice retains the complete `PortContext` at the commerce consumer boundary for the four canonical payment execution calls used by the durable checkout stage executor:

- `prepare_checkout_collection`;
- `authorize_checkout_collection`;
- `capture_checkout_collection`;
- `read_checkout_collection`.

The exact context delegated to `rustok-payment` is cloned for the owner call and retained for diagnostics if that call returns `PortError`.

## Retained diagnostic context

The boundary event records:

- truthful owner `rustok_payment`;
- correlation and tenant identity;
- actor, channel, and locale;
- causation and traceparent;
- idempotency key when the stage is a write;
- owner deadline;
- exact owner operation and commerce payment stage;
- owner code, typed kind, and retryability;
- explicit boundary `commerce_checkout_payment_stage`.

Unavailable, timeout, and invariant failures use error severity. Validation, not-found, conflict, and forbidden rejections use warning severity.

## Preserved contracts

This change does not alter:

- `CheckoutPaymentExecutionPort` requests or responses;
- payment owner provider or lifecycle policy;
- prepare, authorize, capture, or read ordering;
- checkout stage checkpoints or bounded loop behavior;
- collection identity and captured-amount validation;
- `CheckoutPaymentStageError::Boundary` fields;
- public stage, code, message, or retryability propagation;
- payment context correlation, causation, idempotency, locale, or deadline construction.

## Static evidence

`scripts/verify/verify-commerce-checkout-payment-stage-context.mjs` guards:

- all four retained context values and owner delegations;
- exact payment owner operations and commerce stages;
- complete structured diagnostic fields;
- severity classification;
- preservation of the existing boundary payload and checkpoints;
- removal of the former context-dropping `boundary_error` helper;
- absence of inline context construction at the four owner calls.

## Validation status

Tests, Cargo commands, formatting commands, verifier execution, workflow checks, and CI were intentionally not run by the implementation agent, per maintainer instruction.

Intended focused checks:

```bash
node scripts/verify/verify-commerce-checkout-payment-stage-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```

## Remaining work

This slice does not close:

- payment owner-side policy/admission diagnostics;
- checkout payment compensation boundaries;
- GraphQL and HTTP payment query/mutation adapters;
- remaining order, fulfillment, inventory, customer, tax, promotion, and non-`PortError` public envelopes;
- runtime evidence or any FBA/FFA status promotion.
