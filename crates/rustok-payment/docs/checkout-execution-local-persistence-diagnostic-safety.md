# Checkout payment execution local persistence diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

Checkout payment execution can complete authorization or capture at the provider and
then fail while applying the normalized result to local payment state. Both paths first
mark the provider-operation journal as reconciliation-required, emit a private failure
event, and return the common public manual-reconciliation conflict envelope.

The private events previously recorded the complete `PaymentError`. Depending on the
variant, that debug payload could include database details, validation text, UUIDs,
transition values, provider identifiers, or provider operation text.

## Private diagnostics

The authorization and capture events now record only:

- static `PaymentError` variant;
- number and aggregate character length of text fields;
- number and non-nil count of UUID fields;
- whether an opaque database payload is present;
- non-nil operation-ID shape;
- static provider operation (`authorize` or `capture`);
- correlation, stable code, boundary, and existing bounded context facts.

They do not record the complete `PaymentError`, database text, validation text, UUID
values, transition text, provider identifiers, or provider operation payload text.

## Preserved behavior

This source slice does not change:

- provider authorization or capture execution;
- local payment-service calls or their inputs;
- success handling and journal commit ordering;
- the call to `mark_local_persistence_failed` before diagnostics;
- reconciliation-required journal reason construction;
- authorization and capture manual-reconciliation routes;
- the public `payment.checkout_execution_manual_reconciliation` code;
- `PortErrorKind::Conflict`, public message, or retryability;
- payment lifecycle, provider registry behavior, or journal mutation semantics.

The companion reconciliation-reason contract now covers all sixteen checkout execution
manual-reconciliation call sites with stable typed labels.

## Remaining payment execution diagnostics

Separate cleanup remains open for provider checkpoint failures and provider
request/result encoding errors. Those sites still require their own bounded diagnostic
facts and are intentionally unchanged by this slice.

The canonical ecommerce correlation-safe mapper-cleanup item remains open.

## Evidence

- `contracts/evidence/checkout-execution-local-persistence-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-local-persistence-diagnostic-safety.mjs`
- `docs/checkout-execution-reconciliation-reason-diagnostic-safety.md`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
payment owner tests, compilation, and injected authorization/capture local-persistence
failures after successful provider execution.

No payment FFA/FBA, runtime, workflow, CI, or production status is promoted from this
source review.
