# Checkout payment execution local PortError diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

`CheckoutPaymentExecutionPort` exposes four operations through the in-process payment
owner adapter:

- prepare a checkout payment collection;
- authorize the collection;
- capture the collection;
- read the collection for recovery.

Each operation already created bounded request and `PortContext` diagnostic facts before
calling the local implementation. The final local mapper preserved the returned
`PortError`, but both its warning and error events also recorded the complete error and
its message text.

Although technical `PortError` messages are sanitized at construction and serde
boundaries, the mapper also handles validation, not-found, conflict, forbidden, and
other owner-local outcomes. A complete compatibility envelope is not required for
operator correlation and must not become a diagnostic payload.

## Private diagnostics

The final local mapper now records only:

- stable error code;
- static error-kind label;
- retryability;
- error-message presence and character length;
- owner operation and local operation;
- correlation ID;
- existing bounded context and checkout identity facts.

It does not record the complete `PortError` or its message text.

## Preserved behavior

This source slice does not change:

- the four public port methods or their signatures;
- admission, tenant, causation, deadline, or idempotency enforcement;
- payment collection validation or typed lifecycle policy;
- provider selection, authorization, capture, journal, or recovery behavior;
- stable `PortError` codes, kinds, messages, or retryability;
- warning versus error severity selection;
- passthrough of unrecognized local error codes;
- return of the original `PortError` from the mapper.

## Remaining payment execution diagnostics

This is intentionally one bounded sub-slice. Separate cleanup remains open for:

- admission rejection diagnostics;
- complete owner `PaymentError` diagnostics;
- UUID and serde error diagnostics;
- local persistence and provider checkpoint diagnostics;
- manual-reconciliation reason text.

The canonical ecommerce mapper-cleanup item therefore remains open.

## Evidence

- `contracts/evidence/checkout-execution-local-porterror-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-local-porterror-diagnostic-safety.mjs`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
payment owner tests, ecommerce aggregate guards, compilation, and injected failures for
all four operations.

No payment FFA/FBA, transport, runtime, workflow, CI, or production status is promoted
from this source review.
