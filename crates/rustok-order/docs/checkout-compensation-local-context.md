# Order checkout compensation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source slice hardens diagnostics across the canonical
`CheckoutOrderCompensationPort::compensate_checkout_order` path:

- the public local-context wrapper in `checkout_compensation_local_context.rs`;
- shared checkout write admission and context validation in `checkout_owner_context.rs`;
- the canonical compensation owner in `checkout_compensation.rs`.

The public trait, request, snapshot, constructors, factory, compatibility facade, and
Commerce composition remain unchanged.

## Stable-code attribution

The public compensation wrapper classifies known local outcomes from
`PortError.code` only. Human-readable `PortError.message` is not used as control
flow.

Known codes map to:

- `order.checkout_compensation_identity_invalid` → `validate_request`;
- `order.checkout_compensation_identity_conflict` →
  `validate_durable_checkout_identity`;
- `order.checkout_compensation_state_conflict` → `apply_compensation_state`;
- `order.checkout_compensation_manual_reconciliation` →
  `require_manual_reconciliation`.

Both state-conflict messages use one truthful stable label because public wording is
no longer a routing discriminator. Unknown codes pass through without an added local
event. The original delegated `PortError` is returned unchanged.

The shared payment-settlement local mapper in `checkout_owner_context.rs` also uses
stable codes only. This is a diagnostic-only change; payment-settlement owner
business source is unchanged.

## Safe context and request shape

Every compensation layer retains the correlation id. Other `PortContext` values are
represented only by:

- tenant, actor-id, channel, locale, causation-id, traceparent, and idempotency-key
  character lengths;
- actor kind;
- claim and role counts;
- optional-value presence flags;
- deadline milliseconds.

The public wrapper retains request shape before delegation:

- checkout-operation and cart UUID non-nil facts;
- expected-order presence and non-nil state;
- reason presence and character length.

It does not log raw request identifiers or reason text.

## Safe owner identity and lifecycle evidence

The owner still evaluates the same identity rules, but conflict diagnostics now
record comparison facts instead of values:

- tenant match;
- checkout-operation match;
- source-cart match;
- expected-order match;
- UUID presence and non-nil facts.

Cancellation-race diagnostics retain the typed current state and transition labels,
plus only an order UUID non-nil fact. Manual reconciliation retains the typed order
state, optional order-id presence/non-nil state, and the static internal reason.
Order and related-resource lookup diagnostics retain resource kind plus UUID
presence/non-nil state.

Raw tenant, actor, channel, locale, causation, traceparent, idempotency, checkout,
cart, order, durable-identity, and related-resource identifiers are not written by
this compensation boundary.

Original parse, database, core, and owner validation causes remain private to
structured tracing.

## Preserved severity

- write/context/request/not-found/transition rejections remain warning severity;
- durable identity conflicts, database/core failures, and manual reconciliation
  remain error severity.

No public error is reconstructed by the wrapper. Owner code, message, kind, and
retryability mappings remain unchanged.

## Preserved behavior

This slice does not change:

- write policy or write-semantics admission;
- tenant, actor, or causation validation order;
- checkout identity read or legacy adoption;
- the no-identity `Ok(None)` path when no expected order is recorded;
- durable identity comparison rules;
- order loading;
- pending or confirmed cancellation;
- already-cancelled idempotent adoption;
- transition-race reread and cancelled adoption;
- paid, shipped, delivered, or unknown manual-reconciliation routing;
- request/response DTOs;
- order-service calls;
- public `PortError` envelopes;
- Commerce compensation ordering;
- Order FFA/FBA status.

## Static evidence

- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json`
- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source-review.json`
- `scripts/verify/verify-order-compensation-local-context.mjs`
- `scripts/verify/verify-order-checkout-compensation-error-context.mjs`
- `scripts/verify/verify-order-checkout-owner-context.mjs`

The guards cover code-only attribution, safe context/request/identity shape,
forbidden raw fields, unchanged public envelopes, unchanged identity and cancellation
flow, and source-only validation flags.

## Remaining gaps

The broad ecommerce correlation-safe mapper item remains open for remaining Order
settlement owner diagnostics, fulfillment, inventory, customer, tax, promotion,
remaining ecommerce adapters, non-`PortError` envelopes, and runtime evidence.
No architecture status is promoted from source inspection.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-compensation-local-context.mjs
node scripts/verify/verify-order-checkout-compensation-error-context.mjs
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-order-payment-settlement-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
