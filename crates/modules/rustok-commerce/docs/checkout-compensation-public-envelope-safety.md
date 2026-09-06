# Checkout compensation public state-envelope safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the legacy `CheckoutCompensationService` state-derived
`CheckoutCompensationError::ManualReconciliation` and
`CheckoutCompensationError::Conflict` envelopes.

The covered branches include claim contention, captured checkout stages, missing
order identity, unsafe payment-provider operations, payment/order lifecycle
rejections, inventory release and reservation-state mismatches, cart lifecycle
rejections, typed identity mismatch, and unsupported checkout stages.

## Stable public reasons

The seven manual-reconciliation branches and ten conflict branches now use
branch-specific static reasons. They no longer interpolate checkout operation,
order, payment collection, payment-provider operation, inventory reservation,
or cart UUIDs. They also no longer include runtime status or lease-owner values.

The enum variants and their display prefixes remain unchanged:

- `checkout compensation requires manual reconciliation: ...`;
- `checkout compensation conflict: ...`.

The existing compensation error-code classification remains unchanged.

## Preserved execution behavior

This work does not change:

- operation claim and already-compensated replay behavior;
- the captured-stage refund-reconciliation gate;
- order-identity read/adoption and validation;
- provider-operation safety classification;
- payment cancellation eligibility and cancellation call;
- order cancellation eligibility and cancellation call;
- inventory release comparison and journal update;
- cart snapshot/release handling;
- compensation retry journal code selection, message persistence, or mutation
  ordering.

For covered variants, the persisted journal message is still derived from
`compensation.to_string()`, but the state-derived reason is now stable and does
not contain runtime identity or state values.

## Remaining boundary

This slice does not close transparent `CheckoutOperationError`,
`CheckoutInventoryReservationError`, `PaymentError`,
`PaymentOrchestrationError`, or `OrderError` display payloads. It also leaves
`CheckoutCompensationError::Boundary` and
`CheckoutCompensationError::CompensationAndJournal` for separate review.

The broader ecommerce correlation-safe mapper and non-`PortError` public-envelope
task remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/checkout-compensation-public-envelope-safety-source-review.json`
- `scripts/verify/verify-commerce-checkout-compensation-public-envelope-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, workflows, or CI were run.
No compile or runtime status is promoted.
