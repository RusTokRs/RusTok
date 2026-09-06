# Return completion operator envelope safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Admin Return Completion operator boundary used by:

- operation list;
- operation detail;
- explicit retry.

The previous conflict branch classified lease, reconciliation, terminal, replay, command-binding, and command-hash validation text, then returned that internal text directly in the public `409` envelope.

## Bounded public envelope

The operator still evaluates the existing typed `PostOrderOrchestrationError` before projection. The specialized policies remain:

- missing operation: `404` / `return_completion_operation_not_found`;
- leased, reconciliation, terminal, replay, or command conflict: `409` / `return_completion_operation_conflict`;
- order recovery database/core failure: `503` / `return_completion_storage_unavailable`.

The conflict message is now static: `Return completion operation conflicts with the current state`. Internal owner validation text is no longer returned to the client.

## Bounded diagnostics

Each route supplies typed tenant, actor, optional operation identity, and a static operation label. After specialized policy selection and before `tracing::error!`:

- the error becomes a diagnostic type whose `Debug` output is always `redacted`;
- tenant and actor UUIDs become `nil` / `non_nil` shapes;
- optional operation UUID becomes `absent` / `present_nil` / `present_non_nil`;
- owner, source owner, route operation, error kind, public code, HTTP status, boundary, and static event message remain observable.

## Preserved behavior

This work does not change:

- the three mounted routes or their response DTOs;
- `ORDERS_READ` authorization for list/detail;
- combined `ORDERS_MANAGE` and `PAYMENTS_MANAGE` authorization for retry;
- pagination, filtering, totals, and success envelopes;
- `ReturnCompletionOrchestrationService` calls;
- payment-provider registry composition;
- existing internal string classifiers used to distinguish specialized operator states;
- delegation of unmatched errors to the shared post-order mapper.

## Remaining boundary

The owner still represents these operator states through validation text, so replacing internal string classification with typed owner variants remains open. The shared fallback mapper and the broader ecommerce correlation-safe/non-`PortError` cleanup also remain open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/return-completion-operator-envelope-safety-source-review.json`
- `scripts/verify/verify-commerce-return-completion-envelope-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
