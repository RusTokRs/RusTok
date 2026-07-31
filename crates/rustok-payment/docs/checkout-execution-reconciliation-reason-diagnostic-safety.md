# Checkout payment execution reconciliation reason diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

`manual_reconciliation` is the common checkout payment execution helper used when a
durable provider result is missing or malformed and when the payment owner reports an
invalid successful response or an unknown provider outcome.

The helper previously accepted a free-form `&'static str` and wrote that value directly
as `internal_message`. Current call sites used fixed literals, but the parameter shape
allowed future payload-bearing or unstable text to enter private diagnostics.

## Private diagnostics

The helper now accepts a closed four-variant enum:

- `MissingNormalizedDurableResult`;
- `MalformedDurableResult`;
- `InvalidSuccessfulProviderResponse`;
- `UnknownProviderOutcome`.

Each variant maps to one stable snake-case label. Tracing records only that label as
`reconciliation_reason` together with the existing correlation, operation, code,
boundary, deadline, and bounded context facts.

The helper no longer accepts or records free-form reconciliation reason text.

## Preserved behavior

This source slice does not change:

- provider-operation status and result-presence gates;
- durable provider-result decoding or recovery ordering;
- the four existing manual-reconciliation call routes;
- owner error classification;
- the public `payment.checkout_execution_manual_reconciliation` code;
- `PortErrorKind::Conflict`;
- the public message `payment checkout execution requires manual reconciliation`;
- retryability (`false`);
- payment lifecycle, provider execution, journal, or persistence behavior.

## Remaining payment execution diagnostics

Separate cleanup remains open for:

- local persistence diagnostics;
- provider checkpoint diagnostics.

The canonical ecommerce correlation-safe mapper-cleanup item remains open.

## Evidence

- `contracts/evidence/checkout-execution-reconciliation-reason-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-reconciliation-reason-diagnostic-safety.mjs`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
payment owner tests, compilation, and injected execution paths for all four reasons.

No payment FFA/FBA, runtime, workflow, CI, or production status is promoted from this
source review.
