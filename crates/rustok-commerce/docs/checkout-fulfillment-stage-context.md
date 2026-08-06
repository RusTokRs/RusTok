# Checkout fulfillment stage error safety and retry disposition

Status: **source-reviewed / unvalidated**

## Scope

The mounted durable checkout fulfillment stage keeps its established business logic
behind a private retained source and now sanitizes failures from all three typed owner
calls before they reach the stage mapper:

- `CheckoutFulfillmentExecutionPort::ensure_checkout_fulfillments`;
- `CheckoutFulfillmentExecutionPort::read_checkout_fulfillments`;
- `CheckoutOrderPaymentSettlementPort::settle_checkout_payment`.

The complete canonical `PortContext`, requests, successful DTOs, lifecycle policy,
idempotency identities, stage checkpoints, and retry disposition remain unchanged.

## Public and persisted error policy

The exact owner `code`, typed `PortErrorKind`, and `retryable` value are retained.
Owner message text is not copied into `CheckoutFulfillmentStageError::Boundary`, the
staged pipeline error string, or checkout operation journal persistence.

Fulfillment owner failures use these Commerce-owned messages:

| Owner kind | Stage message |
| --- | --- |
| `Validation` | `Checkout fulfillment request is invalid` |
| `NotFound` | `Checkout fulfillment resource was not found` |
| `Conflict` | `Checkout fulfillment state conflicts with the requested operation` |
| `Forbidden` | `Checkout fulfillment operation is not permitted` |
| `Unavailable` / `Timeout` | `Checkout fulfillment service is temporarily unavailable` |
| `InvariantViolation` | `Checkout fulfillment operation could not be completed safely` |

Order payment-settlement failures use the corresponding stable order-settlement
messages, including `Checkout order settlement service is temporarily unavailable`
for unavailable/timeout failures.

## Diagnostic policy

The mounted facade emits only:

- a redacted diagnostic token;
- truthful owner, owner operation, and commerce stage;
- owner code, typed kind, retryability, message presence, and message length;
- bounded tenant/actor identity shapes;
- actor kind plus claim and role counts;
- channel, locale, correlation, causation, trace, and idempotency presence shapes;
- deadline milliseconds;
- the stable `commerce_checkout_fulfillment_execution_adapter` boundary.

Raw `PortError`, owner message text, tenant/actor identities, correlation values,
channel/locale values, causation, traceparent, and idempotency keys are not emitted.

Unavailable, timeout, and invariant failures remain error-level diagnostics.
Validation, not-found, conflict, forbidden, and other ordinary owner rejections remain
warning-level diagnostics.

## Legacy isolation

`checkout_fulfillment_stages_legacy.rs` retains the previously mounted business logic
unchanged. The mounted facade:

1. wraps the canonical fulfillment and order-settlement owner ports;
2. sanitizes owner failures before the retained mapper receives them;
3. suppresses the retained raw compatibility tracing macros;
4. keeps the retained implementation private;
5. preserves canonical custom owner-port injection on
   `CheckoutFulfillmentStageExecutor`.

The retained mapper therefore still preserves stage, code, message, and retryability
structurally, but the message it receives is already the static Commerce-owned message.

## Retry policy

The existing staged-checkout disposition remains unchanged:

- retryable fulfillment ensure/read boundary → `retryable_error`;
- retryable order-payment settlement boundary → `retryable_error`;
- non-retryable fulfillment-stage boundary → `compensation_required`;
- fulfillment-stage conflict → unchanged `compensation_required`;
- fulfillment-stage operation/journal error → unchanged `compensation_required`.

`CheckoutOperationJournal::claim_execution` continues to admit `retryable_error`, and
`RecoveringStagedCheckoutService` continues to invoke synchronous compensation only
for `compensation_required`.

## Resume safety

The owner operations keep their established durable identities:

- fulfillment ensure uses `checkout:{operation_id}:fulfillment-set`;
- fulfillment read remains side-effect free;
- order payment settlement uses
  `checkout:{operation_id}:order:payment-settlement`;
- successful owner outcomes are validated before
  `payment_captured -> fulfillment_created`.

## Preserved contracts

This source wave does not change:

- fulfillment or order owner port traits;
- ensure/read/settlement request or response DTOs;
- owner execution, adoption, settlement, or lifecycle policy;
- fulfillment plan construction or cart-line provenance checks;
- captured-payment admission or amount validation;
- payment-reference/manual-provider fallback behavior;
- idempotency keys, causation, locale, deadlines, or delegated contexts;
- stage checkpoints, operation journal, or recovery implementations;
- pipeline error-code mapping or retry disposition;
- HTTP, GraphQL, or native field/route contracts;
- Commerce FFA/FBA status.

## Source evidence

- `crates/rustok-commerce/contracts/evidence/checkout-fulfillment-stage-error-safety-source-review.json`
- `crates/rustok-commerce/contracts/evidence/checkout-fulfillment-retry-disposition-source-review.json`
- `scripts/verify/verify-commerce-checkout-fulfillment-stage-context.mjs`
- `scripts/verify/verify-commerce-staged-checkout-fulfillment-retry-disposition.mjs`
- `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`

## Remaining work

The broad ecommerce mapper-cleanup P0 remains open for other fulfillment adapters,
tax, promotion, remaining ecommerce adapters, and non-`PortError` public envelopes.
Compile, runtime, database, restart, remote-port, workflow, and CI evidence also
remain open.

## Intended maintainer checks

```bash
node scripts/verify/verify-commerce-checkout-fulfillment-stage-context.mjs
node scripts/verify/verify-commerce-staged-checkout-fulfillment-retry-disposition.mjs
node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs
node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, fulfillment-owner calls,
order-owner calls, database scenarios, restart scenarios, remote-port scenarios,
workflows, or CI were executed.
