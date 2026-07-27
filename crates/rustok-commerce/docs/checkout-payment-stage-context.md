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

## Payment owner local outcomes

The canonical payment execution entrypoint now also retains the accepted `PortContext` and safe request
facts across the unchanged prepare, authorize, capture, and read owner calls. Exact stable identity,
collection, lifecycle, provider, storage, and manual-reconciliation envelopes receive owner-local
diagnostics while returning the same `PortError`.

Potentially unvalidated currency, plan-hash, provider-id, and provider-payment-id strings are represented
only by their character lengths; request metadata is not logged. The complete owner classification and
pass-through contract is documented in
[`../../rustok-payment/docs/checkout-execution-local-context.md`](../../rustok-payment/docs/checkout-execution-local-context.md).

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

`scripts/verify/verify-payment-checkout-execution-local-context.mjs` separately guards owner-side
post-delegation safe-fact retention, stable local-outcome classification, raw-string exclusion, unknown
error pass-through, and same delegated error return.

## Validation status

Tests, Cargo commands, formatting commands, verifier execution, workflow checks, and CI were intentionally not run by the implementation agent, per maintainer instruction.

Intended focused checks:

```bash
node scripts/verify/verify-payment-checkout-execution-local-context.mjs
node scripts/verify/verify-commerce-checkout-payment-stage-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```

## Remaining work

This work does not close:

- payment owner-side policy, tenant, or checkout-operation causation diagnostics;
- checkout payment compensation local outcomes and boundaries;
- GraphQL and HTTP payment query/mutation adapters;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` public envelopes;
- runtime evidence or any FBA/FFA status promotion.
