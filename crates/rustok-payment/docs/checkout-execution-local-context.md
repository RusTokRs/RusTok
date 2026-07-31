# Payment checkout execution diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source slice hardens diagnostics across the canonical
`CheckoutPaymentExecutionPort` implementation for:

- `prepare_checkout_collection`;
- `authorize_checkout_collection`;
- `capture_checkout_collection`;
- `read_checkout_collection`.

It covers admission rejection, tenant and causation validation, provider-result
normalization, manual reconciliation, owner error mapping, and post-delegation local
outcome attribution.

## Stable-code classification

The local outcome mapper now selects `local_operation` from `PortError.code` plus the
known owner operation where an authorize/capture distinction is required. Human-readable
`PortError.message` is not used as control flow.

This keeps diagnostic attribution stable when public wording changes while preserving the
same delegated `PortError` code, message, kind, and retryability.

Unknown codes continue to pass through without an added local outcome event.

## Safe diagnostic context

The per-call correlation id remains available for event joining. Other `PortContext`
values are recorded only as non-sensitive shape facts:

- tenant, actor-id, channel, locale, causation-id, traceparent, and idempotency-key
  character lengths;
- actor kind;
- claim and role counts;
- presence flags for optional values;
- deadline milliseconds.

The diagnostics do not record raw tenant ids, actor ids, channels, locales, causation ids,
traceparents, or idempotency keys.

Request identity is also shape-only:

- non-nil flags for checkout operation, cart, order, customer, and collection UUIDs;
- presence flags for optional customer, collection, provider, and provider-payment ids;
- decimal text length rather than the requested amount;
- currency, order-plan-hash, provider-id, and provider-payment-id character lengths.

Raw checkout/payment identifiers, provider identities, request metadata, and financial
values are not written by these execution diagnostics.

## Preserved behavior

This slice does not change:

- `CheckoutPaymentExecutionPort` or request/response DTOs;
- read/write policy, deadline, idempotency, tenant, or causation admission;
- prepare, authorize, capture, or read delegation order;
- payment collection validation or lifecycle policy;
- provider selection, authorize/capture payloads, canonical provider idempotency keys,
  journal claim/checkpoint/replay, or reconciliation policy;
- `PaymentError` to public `PortError` mapping;
- public codes, messages, kinds, or retryability;
- successful results or replay adoption;
- payment compensation source or contracts;
- Payment FFA/FBA status.

The original internal error remains private to structured tracing.

## Static evidence

- `crates/rustok-payment/contracts/evidence/checkout-execution-diagnostic-safety-source.json`
- `crates/rustok-payment/contracts/evidence/checkout-execution-diagnostic-safety-source-review.json`
- `scripts/verify/verify-payment-checkout-execution-local-context.mjs`

The verifier guards code-only classification, safe context/request shape, forbidden raw
value logging, unchanged public envelopes, unchanged owner delegation, and source-only
validation flags.

## Remaining gaps

Payment checkout compensation still uses message-pair classification and raw context/
identifier diagnostics. It remains the next separate owner-boundary cleanup.

The broad ecommerce correlation-safe mapper item also remains open for remaining owner
adapters, non-`PortError` envelopes, and runtime evidence. No FBA or FFA status is
promoted from source inspection.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-checkout-execution-local-context.mjs
node scripts/verify/verify-payment-checkout-execution-error-safety.mjs
node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
