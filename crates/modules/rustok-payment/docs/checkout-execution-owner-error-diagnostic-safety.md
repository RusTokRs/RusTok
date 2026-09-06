# Checkout payment execution owner error diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

`payment_error_to_port_error` is the payment owner mapper used by checkout execution
storage, collection, lifecycle, and provider paths. It selects a stable internal code,
records a private diagnostic event, and then converts one of eleven `PaymentError`
variants to the existing public `PortError` contract.

The mapper previously recorded the complete `PaymentError`. Depending on the variant,
that debug payload could contain database details, validation text, collection/payment/
refund UUIDs, transition values, provider IDs, or provider operation text.

## Private diagnostics

The owner mapper now records only:

- stable owner error code;
- static `PaymentError` variant;
- count and aggregate character length of text fields;
- count and non-nil count of UUID fields;
- whether an opaque database payload is present;
- owner operation, correlation ID, and existing bounded context facts.

It does not record the complete `PaymentError`, database text, validation text, UUID
values, transition values, provider IDs, or provider operation text.

## Preserved behavior

This source slice does not change:

- `PaymentError` or `PortError` public types;
- stable owner code selection;
- any of the eleven conversion match arms;
- public error codes, kinds, messages, or retryability;
- database-unavailable and validation mapping;
- not-found mapping for collections, payments, and refunds;
- invalid-transition conflict mapping;
- provider unavailable/rejected/configuration mapping;
- manual-reconciliation routing for invalid responses and unknown outcomes;
- provider retry/reconciliation policy, journal behavior, or payment lifecycle.

## Remaining payment execution diagnostics

Separate cleanup remains open for:

- UUID and serde error diagnostics;
- manual-reconciliation reason text;
- local persistence diagnostics;
- provider checkpoint diagnostics.

The canonical ecommerce correlation-safe mapper-cleanup item remains open.

## Evidence

- `contracts/evidence/checkout-execution-owner-error-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-owner-error-diagnostic-safety.mjs`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
payment owner tests, ecommerce aggregate guards, compilation, and injected failures for
all eleven owner error variants.

No payment FFA/FBA, runtime, workflow, CI, or production status is promoted from this
source review.
