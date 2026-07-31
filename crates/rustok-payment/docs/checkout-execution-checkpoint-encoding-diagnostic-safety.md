# Checkout payment execution checkpoint and encoding diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

Checkout payment execution retains six private failure events around durable journal
checkpoints and JSON request/result encoding:

- local commit checkpoint failure;
- local reconciliation-required checkpoint failure;
- provider-failure checkpoint failure;
- provider-success checkpoint failure;
- provider request encoding failure;
- provider result encoding failure.

The four checkpoint events previously recorded complete `PaymentError` debug payloads.
The two encoding events previously recorded complete serde errors. Those payloads could
contain database details, validation text, identities, transition/provider values, or
unstable parser/serializer text.

## Private diagnostics

The four checkpoint events now retain only:

- static `PaymentError` variant;
- number and aggregate character length of text fields;
- number and non-nil count of UUID fields;
- whether an opaque database payload is present;
- operation-ID non-nil shape;
- provider operation and provider-ID length where already present;
- correlation, stable code, boundary, and bounded context facts.

The request and result encoding events now retain only a static failure flag plus their
existing bounded operation/context facts. They do not record serde error text or the
request/result payload.

No event records the complete `PaymentError`, database payload, validation text, UUID
value, provider payload text, or serde error.

## Preserved behavior

This source slice does not change:

- request construction or `serde_json::to_value` call placement;
- provider authorization/capture dispatch;
- provider error classification through `requires_provider_reconciliation`;
- `mark_provider_error`, `mark_reconciliation_required`, `mark_provider_succeeded`, or
  `mark_committed` call order and inputs;
- local reconciliation reason construction;
- all six stable diagnostic codes or tracing severity;
- typed manual-reconciliation reasons;
- the request-encoding invariant-violation code/message;
- the public manual-reconciliation conflict code, kind, message, or retryability;
- payment lifecycle, provider registry behavior, or journal mutation semantics.

## Checkout execution source status

Together with the admission, owner-error, UUID/serde decode, reconciliation-reason, and
local-persistence source contracts, this slice closes the currently identified checkout
execution payload-diagnostic sites at source level.

This is not compile, test, runtime, workflow, CI, or production evidence. The canonical
ecommerce correlation-safe mapper-cleanup item remains open across other modules and
public envelopes.

## Evidence

- `contracts/evidence/checkout-execution-checkpoint-encoding-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-checkpoint-encoding-diagnostic-safety.mjs`
- `docs/checkout-execution-local-persistence-diagnostic-safety.md`
- `docs/checkout-execution-reconciliation-reason-diagnostic-safety.md`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
compilation, and injected failures for all four checkpoint and both encoding paths.

No payment FFA/FBA, runtime, workflow, CI, or production status is promoted from this
source review.
