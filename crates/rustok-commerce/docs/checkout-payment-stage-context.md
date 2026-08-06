# Checkout payment stage error safety

Status: **source-reviewed / unvalidated**

## Scope

This source wave closes the payment-execution mapper gap on the mounted durable
checkout path.

`checkout_payment_stages.rs` is now a thin facade over the retained
`checkout_payment_stages_legacy.rs` implementation. The legacy implementation
keeps the established prepare, authorize, capture, read, checkpoint, replay, and
collection-validation behavior unchanged. A payment-port adapter now intercepts
owner failures before they reach the legacy stage mapper.

The adapter covers the four canonical `CheckoutPaymentExecutionPort` calls:

- `prepare_checkout_collection`;
- `authorize_checkout_collection`;
- `capture_checkout_collection`;
- `read_checkout_collection`.

## Public and persisted error policy

The owner code and retryability remain unchanged. The owner message is not copied
into `CheckoutPaymentStageError::Boundary`, the staged pipeline error string, or
the checkout operation journal.

| Owner kind | Stage message |
| --- | --- |
| `Validation` | `Checkout payment request is invalid` |
| `NotFound` | `Checkout payment resource was not found` |
| `Conflict` | `Checkout payment state conflicts with the requested operation` |
| `Forbidden` | `Checkout payment operation is not permitted` |
| `Unavailable` / `Timeout` | `Checkout payment service is temporarily unavailable` |
| `InvariantViolation` | `Checkout payment operation could not be completed safely` |

The existing boundary variant remains structurally compatible:

- `stage` remains the commerce payment stage;
- `code` remains the payment owner code;
- `message` is now the static kind-based stage message;
- `retryable` remains the payment owner value.

This wave intentionally does not change staged-checkout failure disposition.
Existing pipeline persistence and compensation admission continue to classify the
same error variants as before.

## Context propagation

The complete canonical `PortContext` is still delegated to the payment owner.
Correlation, tenant, actor, channel, locale, causation, trace, idempotency, and
deadline values are therefore preserved for owner policy and remote-adapter
compatibility.

The Commerce consumer boundary no longer logs those raw values. It records only:

- a diagnostic token whose `Debug` output is `redacted`;
- owner operation, owner code, typed kind, and retryability;
- tenant and actor identity shapes;
- actor kind plus claim and role counts;
- channel, locale, correlation, causation, trace, and idempotency presence shapes;
- deadline milliseconds;
- owner-message presence and character length;
- the stable boundary `commerce_checkout_payment_execution_adapter`.

Unavailable, timeout, and invariant failures use error severity. Validation,
not-found, conflict, and forbidden outcomes use warning severity.

## Legacy isolation

The retained legacy source still contains its former local mapper and logger, but
the mounted facade:

1. supplies a sanitizing payment-port implementation;
2. converts canonical `PortError` into a bounded private error before delegation
   returns to the legacy stage;
3. shadows the legacy tracing macros so the old raw-context event is not emitted;
4. keeps the legacy module private;
5. exposes the same public executor methods and canonical
   `with_payment_port(Arc<dyn CheckoutPaymentExecutionPort>)` builder.

The retained source blob is copied without business-logic edits. It is not a
second mounted implementation.

## Preserved contracts

This wave does not alter:

- payment owner requests, responses, provider policy, or lifecycle policy;
- prepare, authorize, capture, or read ordering;
- write idempotency keys, causation, locale, or deadlines;
- checkout stage checkpoints or bounded loop behavior;
- collection identity, status, or captured-amount validation;
- successful checkout DTOs;
- public executor method names or canonical custom-port injection;
- HTTP, GraphQL, or native field/route contracts;
- Commerce FFA/FBA status.

## Evidence

- `crates/rustok-commerce/contracts/evidence/checkout-payment-stage-error-safety-source-review.json`
- `scripts/verify/verify-commerce-checkout-payment-stage-context.mjs`

## Remaining work

- Execute the focused verifier and compile the Commerce library.
- Exercise prepare, authorize, capture, and read failure scenarios and inspect the
  checkout operation journal.
- Continue payment compensation, order, fulfillment, inventory, promotion, and
  remaining non-`PortError` mapper cleanup.
- Remove the retained legacy stage source only after compile, replay, mounted
  parity, and upgraded-path evidence.

## Intended checks

```bash
node scripts/verify/verify-commerce-checkout-payment-stage-context.mjs
cargo check -p rustok-commerce --lib
```

No tests, Node verifiers, Cargo commands, formatting, payment-provider calls,
database scenarios, workflows, or CI were executed.
