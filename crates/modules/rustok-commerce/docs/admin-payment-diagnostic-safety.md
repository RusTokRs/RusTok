# Admin payment diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the shared failure diagnostic used by the ten mounted Commerce Admin Payment routes:

- payment-collection list and detail reads;
- authorize, capture, and cancel collection mutations;
- idempotent refund creation;
- refund list and detail reads;
- refund complete and cancel mutations.

The owner and orchestration mappers continue to select typed HTTP policy and adopt truthful resource identity before delegating to the shared HTTP helper. Only the diagnostic projection emitted by that helper changes.

## Bounded diagnostic projection

Typed `PaymentError`, `PaymentOrchestrationError`, and `AdminPaymentErrorContext` remain available while policy and identity decisions are made. Before `tracing::error!`:

- the typed error is shadowed by a diagnostic type whose `Debug` output is always `redacted`;
- tenant and actor identifiers become `nil` / `non_nil` shapes;
- optional payment-collection, refund, order, cart, and customer identifiers become `absent` / `present_nil` / `present_non_nil` shapes;
- source owner, static operation, error kind, public code, HTTP status, boundary, and static event message remain visible.

No validation text, database cause, provider payload, transition detail, orchestration source, or UUID is emitted by this helper.

## Preserved behavior

This work does not change:

- the exhaustive `PaymentError` HTTP policy;
- the reserved-refund reconciliation and safe-retry policy;
- adoption of identifiers carried by payment-collection, payment, and refund not-found variants;
- adoption of the reserved refund identifier after provider execution divergence;
- `HttpError::new(status, code, message)` construction;
- `PAYMENTS_READ` and `PAYMENTS_UPDATE` permissions;
- filters, pagination, DTO inputs, or response envelopes;
- direct `PaymentService` reads;
- provider-registry-backed `PaymentOrchestrationService` calls;
- refund `Idempotency-Key` validation and maximum-length policy;
- success contracts for all ten routes.

## Remaining boundary

The broad ecommerce correlation-safe mapper and non-`PortError` public-envelope cleanup remains open. This slice does not claim completion for shipping administration, checkout-operation administration, reconciliation transports, storefront/native/GraphQL boundaries, owner adapters, or runtime verification.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-payment-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-payment-diagnostic-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
