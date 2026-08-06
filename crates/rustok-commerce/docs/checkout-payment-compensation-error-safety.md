# Checkout payment compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

This slice closes raw payment-owner error propagation at the mounted Commerce checkout compensation consumer boundary.

`services/mod.rs` mounts `checkout_compensation_payment_safe.rs`. The facade retains the established compensation implementation through a private `include!` of `checkout_compensation_owner_ports.rs`, while adapting every composed `CheckoutPaymentCompensationPort` before the legacy service receives an error.

The default in-process factory, provider-registry construction, and custom payment compensation port injection all pass through the same adapter. The complete canonical `PortContext` and request are still delegated to `rustok-payment` unchanged.

Order, inventory, and cart compensation boundaries are intentionally outside this slice and continue through the retained implementation without payment-adapter policy.

## Public and persisted messages

The payment compensation adapter preserves the exact owner `kind`, `code`, and `retryable` fields while replacing owner message text with static Commerce-owned messages:

- validation: `Checkout payment compensation request is invalid`;
- not found: `Checkout payment compensation resource was not found`;
- conflict: `Checkout payment compensation conflicts with the current payment state`;
- forbidden: `Checkout payment compensation is not permitted`;
- unavailable or timeout: `Checkout payment compensation service is temporarily unavailable`;
- invariant violation: `Checkout payment compensation could not be completed safely`;
- `payment.checkout_compensation_manual_reconciliation`: `Checkout payment compensation requires manual reconciliation`.

The retained mapper can continue classifying the stable manual-reconciliation code and constructing the existing `CheckoutCompensationError` variants, but it receives only the static adapter message. Because `CheckoutCompensationService::compensate` persists `compensation.to_string()`, the original payment-owner message can no longer reach `mark_compensation_retryable`.

## Bounded diagnostics

The adapter records only:

- truthful owner, owner operation, Commerce stage, boundary identity, stable owner code, typed kind, and retryability;
- whether the owner message exists and its character length, never its text;
- tenant and actor identity shapes;
- actor kind, claim count, and role count;
- channel, locale, correlation, causation, traceparent, and idempotency presence or shape;
- deadline semantics;
- checkout-operation and optional collection UUID shapes;
- reason presence and length;
- metadata JSON kind and top-level entry count;
- a redacted diagnostic token.

Unavailable, timeout, and invariant failures use error severity. Validation, not-found, conflict, and forbidden outcomes use warning severity.

The facade suppresses the retained compatibility diagnostic only when its truthful owner is `rustok_payment`; non-payment compensation diagnostics continue unchanged. The payment diagnostic does not record the complete `PortError`, raw request payload, raw tenant or actor identifiers, raw correlation or causation values, raw idempotency keys, or owner message text.

## Preserved contracts

This change does not alter:

- `CheckoutPaymentCompensationPort` requests, responses, or owner delegation;
- payment provider cancellation or manual-reconciliation policy;
- compensation ordering across payment, order, inventory, and cart;
- operation claim, retryable journal update, or compensated checkpoint behavior;
- successful compensation DTOs;
- owner error kind, code, or retryability propagation;
- custom payment compensation port injection or provider registry composition;
- retained order, inventory, or cart compensation implementation;
- GraphQL, HTTP, native, FBA, or FFA status.

## Static evidence

`scripts/verify/verify-commerce-checkout-payment-compensation-error-safety.mjs` guards the mounted facade, all three payment-port composition paths, static message policy, owner classification preservation, bounded diagnostics, payment-only compatibility-log suppression, journal-path safety, truthful evidence, and the intentionally unchanged retained implementation.

The existing `verify-commerce-checkout-compensation-owner-boundary.mjs` now reviews the mounted facade together with the retained source when checking owner-port topology.

## Validation status

Tests, Node verifiers, Cargo commands, formatting commands, payment-provider calls, database scenarios, workflows, and CI were intentionally not run by the implementation agent, per maintainer instruction.

Suggested maintainer checks:

```bash
node scripts/verify/verify-commerce-checkout-payment-compensation-error-safety.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-payment-checkout-compensation-wrapper-error-diagnostic-safety.mjs
cargo check -p rustok-commerce --lib
```

## Remaining work

This source review does not close:

- order, inventory, or cart compensation consumer mapper message and context cleanup;
- non-payment `CheckoutCompensationError` persistence cleanup;
- runtime, provider, database, restart, remote-port, workflow, or CI evidence;
- broad ecommerce mapper cleanup or any FBA/FFA promotion.
