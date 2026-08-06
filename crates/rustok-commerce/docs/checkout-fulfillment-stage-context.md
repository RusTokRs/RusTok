# Checkout fulfillment stage owner context and retry disposition

Status: **source-reviewed / unvalidated**

## Scope

The mounted durable checkout fulfillment stage makes three typed owner calls:

- `CheckoutFulfillmentExecutionPort::ensure_checkout_fulfillments`;
- `CheckoutFulfillmentExecutionPort::read_checkout_fulfillments`;
- `CheckoutOrderPaymentSettlementPort::settle_checkout_payment`.

The stage preserves the complete delegated `PortContext` and maps owner failures to
`CheckoutFulfillmentStageError::Boundary`, including the exact owner code and
`retryable` value.

This slice fixes the staged-checkout disposition of that existing typed boundary.
A fulfillment-stage boundary with `retryable: true` is now persisted through the
existing `mark_retryable_error` transition instead of being marked
`compensation_required`.

## Retry policy

The source policy is intentionally narrow:

- retryable fulfillment ensure/read boundary → `retryable_error`;
- retryable order-payment settlement boundary → `retryable_error`;
- non-retryable fulfillment-stage boundary → `compensation_required`;
- fulfillment-stage conflict → unchanged `compensation_required`;
- fulfillment-stage operation/journal error → unchanged `compensation_required`.

`CheckoutOperationJournal::claim_execution` already admits `retryable_error`, so a
later checkout attempt can reclaim the operation and resume from the durable stage.
`RecoveringStagedCheckoutService` remains unchanged and invokes synchronous
compensation only when the operation is `compensation_required`.

## Resume safety

The existing owner operations remain suitable for durable resume:

- fulfillment ensure uses the canonical
  `checkout:{operation_id}:fulfillment-set` idempotency key;
- fulfillment read is side-effect free;
- order payment settlement uses the canonical
  `checkout:{operation_id}:order:payment-settlement` idempotency key;
- successful owner outcomes are still validated before the
  `payment_captured -> fulfillment_created` checkpoint.

## Preserved contracts

This slice does not change:

- fulfillment or order owner port traits;
- ensure/read/settlement request and response DTOs;
- owner execution, adoption, settlement, or lifecycle policy;
- fulfillment plan construction and cart-line provenance checks;
- captured-payment admission and amount validation;
- payment-reference or manual-provider fallback behavior;
- idempotency keys, causation, locale, deadlines, or delegated contexts;
- bounded stage-loop behavior;
- stage checkpoints;
- operation journal implementation;
- recovery service implementation;
- pipeline error-code mapping;
- public errors or HTTP/GraphQL/native contracts;
- FBA or FFA status.

## Source evidence

- `crates/rustok-commerce/contracts/evidence/checkout-fulfillment-retry-disposition-source-review.json`
- `scripts/verify/verify-commerce-checkout-fulfillment-stage-context.mjs`
- `scripts/verify/verify-commerce-staged-checkout-fulfillment-retry-disposition.mjs`

## Remaining work

The broader fulfillment mapper cleanup remains open, including raw diagnostic and
public-envelope review outside this disposition slice. Compile, runtime, database,
restart, remote-port, workflow, and CI evidence also remain open.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-fulfillment-stage-context.mjs
node scripts/verify/verify-commerce-staged-checkout-fulfillment-retry-disposition.mjs
cargo test -p rustok-commerce staged_checkout::tests::retryable_fulfillment_stage_boundary_does_not_force_compensation
cargo test -p rustok-commerce staged_checkout::tests::retryable_order_settlement_boundary_does_not_force_compensation
cargo test -p rustok-commerce staged_checkout::tests::non_retryable_fulfillment_stage_boundary_requires_compensation
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, fulfillment-owner calls,
order-owner calls, database scenarios, restart scenarios, remote-port scenarios,
workflows, or CI were executed.
