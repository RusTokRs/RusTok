# Checkout payment stage error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted payment-execution facade covers the four canonical
`CheckoutPaymentExecutionPort` calls:

- `prepare_checkout_collection`;
- `authorize_checkout_collection`;
- `capture_checkout_collection`;
- `read_checkout_collection`.

The facade continues to preserve the complete delegated `PortContext`, canonical
requests and successful responses, exact owner code, typed kind, and owner
retryability while replacing owner message text with a static Commerce message.

This source slice additionally corrects staged-checkout failure disposition for
retryable payment-stage owner boundaries.

## Retry disposition

`CheckoutStagePipelineError::PaymentStage` now maps to
`FailureDisposition::Retryable` only when the contained
`CheckoutPaymentStageError::Boundary` has `retryable: true`.

That disposition uses the existing `mark_retryable_error` journal transition. A
later checkout attempt can claim the persisted `retryable_error` operation and
resume from its current durable stage. `RecoveringStagedCheckoutService` does not
start synchronous compensation for that status.

Non-retryable payment-stage failures still require compensation. Payment-stage
conflicts, operation errors, and boundary errors with `retryable: false` continue
through the fail-closed `CompensationRequired` fallback.

## Preserved payment behavior

This slice does not change:

- payment owner prepare, authorize, capture, or read execution;
- provider-operation identities or immutable request payloads;
- owner lifecycle and replay policy;
- payment stage checkpoint ordering;
- payment collection identity, status, or captured-amount validation;
- the operation journal implementation;
- the recovery service implementation;
- the payment pipeline error-code mapping;
- HTTP, GraphQL, or native contracts;
- Commerce FFA or FBA status.

The retryable payment boundary remains visible to the caller as the same
`CheckoutStagePipelineError::PaymentStage` value. Only the persisted operation
status changes from `compensation_required` to `retryable_error` for the explicitly
retryable boundary case.

## Public and persisted message policy

The owner message is not copied into `CheckoutPaymentStageError::Boundary`, the
staged pipeline error string, or the checkout operation journal. Static messages
remain selected from `PortErrorKind`:

| Owner kind | Stage message |
| --- | --- |
| `Validation` | `Checkout payment request is invalid` |
| `NotFound` | `Checkout payment resource was not found` |
| `Conflict` | `Checkout payment state conflicts with the requested operation` |
| `Forbidden` | `Checkout payment operation is not permitted` |
| `Unavailable` / `Timeout` | `Checkout payment service is temporarily unavailable` |
| `InvariantViolation` | `Checkout payment operation could not be completed safely` |

## Source evidence

- `crates/rustok-commerce/contracts/evidence/checkout-payment-stage-error-safety-source-review.json`
- `scripts/verify/verify-commerce-checkout-payment-stage-context.mjs`
- `scripts/verify/verify-commerce-staged-checkout-payment-retry-disposition.mjs`

The source includes focused tests for both sides of the policy:

- retryable payment boundary → `Retryable`;
- non-retryable payment boundary → `CompensationRequired`.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-payment-stage-context.mjs
node scripts/verify/verify-commerce-staged-checkout-payment-retry-disposition.mjs
cargo test -p rustok-commerce staged_checkout::tests::retryable_payment_stage_boundary_does_not_force_compensation
cargo test -p rustok-commerce staged_checkout::tests::non_retryable_payment_stage_boundary_requires_compensation
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, provider calls, database
scenarios, restart scenarios, remote-port scenarios, workflows, or CI were
executed. Compile and runtime behavior are not claimed.
