# Checkout payment execution reconciliation reason diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

`manual_reconciliation` is the common checkout payment execution helper used after
externally visible provider work can no longer be represented as an ordinary retryable
owner error. Its call sites span durable-result recovery, owner error conversion,
provider identity enrichment, authorization and capture lifecycle handling, local
persistence failure, and provider journal checkpoint and encoding failure.

The helper accepts only `CheckoutPaymentExecutionReconciliationReason`; it cannot accept
or record free-form reason text.

## Private diagnostics

The closed enum contains sixteen reasons grouped across:

- missing or malformed durable provider results;
- invalid successful responses and unknown provider outcomes;
- missing or incomplete durable authorize/provider identity;
- authorization and capture unknown-lifecycle or local-persistence failure;
- in-progress/reconciliation-required provider execution;
- commit, provider-failure, result-encoding, and provider-success checkpoint failure.

Each variant maps to one stable snake-case label. Tracing records only that label as
`reconciliation_reason` together with correlation, operation, code, boundary, deadline,
and bounded context facts.

All sixteen checkout execution call sites use typed variants; no call site passes a
string reason.

## Preserved behavior

This source contract does not change:

- provider-operation status and result-presence gates;
- durable provider-result decoding or recovery ordering;
- authorization, capture, or provider execution ordering;
- any manual-reconciliation call route;
- the public `payment.checkout_execution_manual_reconciliation` code;
- `PortErrorKind::Conflict`;
- the public message `payment checkout execution requires manual reconciliation`;
- retryability (`false`);
- journal mutation or persistence behavior.

## Related diagnostic slices

Authorization and capture local-persistence `PaymentError` payload logging is sanitized
in `checkout-execution-local-persistence-diagnostic-safety.md`.

Provider checkpoint and request/result encoding diagnostics are sanitized in
`checkout-execution-checkpoint-encoding-diagnostic-safety.md`. The currently identified
checkout execution payload-diagnostic sites are therefore source-closed, while all
execution, compilation, workflow, CI, and production evidence remains unvalidated.

The canonical ecommerce correlation-safe mapper-cleanup item remains open across other
modules and public envelopes.

## Evidence

- `contracts/evidence/checkout-execution-reconciliation-reason-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-reconciliation-reason-diagnostic-safety.mjs`
- `docs/checkout-execution-local-persistence-diagnostic-safety.md`
- `docs/checkout-execution-checkpoint-encoding-diagnostic-safety.md`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
payment owner tests, compilation, and injected execution paths for all sixteen reasons.

No payment FFA/FBA, runtime, workflow, CI, or production status is promoted from this
source review.
