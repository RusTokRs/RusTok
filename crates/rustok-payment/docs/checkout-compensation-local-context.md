# Payment checkout compensation wrapper diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the public context wrapper around the canonical payment compensation owner boundary:

- `CheckoutPaymentCompensationPort::compensate_checkout_payment`;
- public `InProcessCheckoutPaymentCompensationPort`;
- root `in_process_checkout_payment_compensation_port`;
- the compatibility namespace `rustok_payment::checkout_compensation::*`.

The wrapper still delegates the original `PortContext` and request to the private persistent owner in
`checkout_compensation.rs`. Provider cancellation, journal adoption, lifecycle policy, local
cancellation, and reconciliation behavior remain in that persistent owner and are unchanged here.

## Public construction

`lib.rs` preserves the existing layered facade:

- `checkout_compensation.rs` is loaded privately as `checkout_compensation_persistent`;
- `checkout_compensation_context.rs` owns the public wrapper implementation;
- `checkout_compensation_api.rs` preserves the public module namespace;
- crate-root exports resolve to the wrapper type and factory.

Callers therefore continue to use the same public trait, request, type, and factory names without a
public persistent-implementation bypass.

## Delegation order

The public operation keeps this source order:

1. clone the incoming `PortContext` for post-delegation diagnostics;
2. retain safe request-shape facts;
3. delegate the original context and request to the persistent owner;
4. inspect only a returned `PortError`;
5. classify known outcomes from stable `PortError.code`;
6. return the same `PortError` unchanged.

Human-readable `PortError.message` is not used as control flow. Unknown codes pass through without an
additional wrapper event.

The two previous message-specific labels behind
`payment.checkout_compensation_state_conflict` are intentionally represented by one stable wrapper
label, `apply_compensation_state`, because the public message is no longer a routing discriminator.

## Safe wrapper context

The per-call correlation id remains available for joining diagnostics. Other `PortContext` values are
recorded only as shape facts:

- tenant, actor-id, channel, locale, causation-id, traceparent, and idempotency-key lengths;
- actor kind;
- claim and role counts;
- presence flags for optional values;
- deadline milliseconds.

The wrapper no longer records raw tenant id, actor id, channel, locale, causation id, traceparent, or
idempotency key.

Request facts are also shape-only:

- checkout-operation UUID non-nil state;
- collection-id presence and non-nil state;
- reason presence and character length;
- metadata JSON kind;
- object-field or array-element count when applicable.

The wrapper does not log checkout-operation ids, collection ids, reason text, or metadata values.
The original typed `PortError` remains private to structured tracing.

## Stable-code attribution

The wrapper recognizes stable codes for:

- write idempotency and deadline admission;
- invalid compensation identity;
- collection, payment, and refund lookup;
- manual reconciliation;
- compensation lifecycle/state conflicts;
- unsupported provider-journal state;
- provider metadata and provider identity validation;
- provider cancel request encoding;
- payment storage and owner validation;
- provider unavailable, rejected, invalid response, and missing configuration outcomes.

Unavailable, timeout, and invariant kinds use error severity. Manual reconciliation and unsupported
provider-journal state also use error severity. Other recognized outcomes use warning severity.

No public error is reconstructed. Code, message, kind, retryability, and return identity remain those
of the delegated `PortError`.

## Preserved persistent-owner behavior

This slice does not change:

- compensation request or response DTOs;
- write policy, deadline, idempotency, tenant, or causation admission;
- optional missing-collection no-op behavior;
- captured-payment refund-policy reconciliation;
- payment collection lifecycle classification;
- provider selection or manual-provider fallback;
- canonical `payment_collection:{collection_id}:cancel` idempotency key;
- provider request metadata and `cancel_payment_collection` marker;
- provider journal begin, claim, replay, error, success, commit, or reconciliation transitions;
- provider payment identity recovery;
- local cancellation race handling;
- final provider-operation commit checkpoint;
- provider-registry constructor behavior;
- Commerce compensation ordering;
- public `PortError` envelopes;
- Payment FFA/FBA status.

## Static evidence

- `crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json`
- `crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json`
- `scripts/verify/verify-payment-checkout-compensation-local-context.mjs`

The focused verifier guards wrapper/facade construction, context and request shape, stable-code-only
classification, absence of raw wrapper fields, unchanged persistent owner markers, same-error return,
and source-only validation flags.

## Remaining gaps

The persistent owner in `checkout_compensation.rs` still contains raw tenant, causation,
checkout-operation, collection, and provider-journal operation identifiers in owner-local diagnostics.
It remains the next separate Payment compensation diagnostic-safety slice.

The broad ecommerce correlation-safe mapper item also remains open for remaining owner adapters,
non-`PortError` envelopes, and runtime evidence. No FBA or FFA status is promoted from source
inspection.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-checkout-compensation-local-context.mjs
node scripts/verify/verify-payment-checkout-execution-local-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
