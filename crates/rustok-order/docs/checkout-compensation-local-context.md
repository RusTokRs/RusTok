# Order checkout compensation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source boundary covers the canonical
`CheckoutOrderCompensationPort::compensate_checkout_order` path:

- the public local-context wrapper in `checkout_compensation_local_context.rs`;
- shared checkout write admission and context validation in `checkout_owner_context.rs`;
- the canonical compensation owner in `checkout_compensation.rs`.

The public trait, request, snapshot, constructors, factory, compatibility facade, and
Commerce composition remain unchanged.

## Public wrapper diagnostics

The public compensation wrapper still classifies known local outcomes from
`PortError.code` only. Human-readable `PortError.message` is not used as control
flow, unknown codes pass through without another event, and the same delegated
`PortError` is returned unchanged.

Known codes still map to:

- `order.checkout_compensation_identity_invalid` → `validate_request`;
- `order.checkout_compensation_identity_conflict` →
  `validate_durable_checkout_identity`;
- `order.checkout_compensation_state_conflict` → `apply_compensation_state`;
- `order.checkout_compensation_manual_reconciliation` →
  `require_manual_reconciliation`.

Both wrapper severity branches now retain only:

- stable `PortError.code`;
- a closed static `PortErrorKind` label;
- message presence and character length;
- retryability;
- safe context and request shape.

They do not record the complete `PortError`, its debug representation, or message
text.

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

## Owner error payload shape

The seven `OrderError` variants now retain only:

- a closed static variant label;
- aggregate text-field count and total character length;
- aggregate UUID-field count and non-nil count;
- an opaque-payload presence flag for database and core failures;
- the established stable code, local operation, correlation id, and safe context
  shape.

Validation causes, transition labels, UUID values, database errors, and core errors
are not written into owner events. Order and related-resource lookup events retain a
static resource kind plus aggregate error shape.

Tenant and actor UUID parse failures retain only the static field name, supplied
value length, and `parse_failed = true`. Parser error text is not logged.

## Lifecycle and reconciliation shape

Cancellation-race diagnostics retain:

- order UUID non-nil state;
- a closed static current lifecycle label;
- transition source/target presence and character lengths.

They do not retain raw transition text.

Manual reconciliation retains:

- optional order-id presence and non-nil state;
- a closed static order lifecycle label;
- reconciliation-reason presence and character length.

It does not retain internal reason text. All three existing reconciliation routes
and the public conflict envelope remain unchanged.

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

The two compensation guards cover stable-code wrapper attribution, closed
`PortError` and `OrderError` shape, static parse/lifecycle/reconciliation facts,
forbidden payload values, unchanged public envelopes, unchanged identity and
cancellation flow, and source-only validation flags.

## Source status and remaining gaps

The currently identified checkout order compensation payload-diagnostic sites are
**source-closed / unvalidated**. Compile, compensation replay, process-exit, restart,
contention, mounted transport, remote-profile, workflow, CI, and production evidence
remain unexecuted.

The broad ecommerce correlation-safe mapper item remains open for remaining Order
settlement diagnostics, fulfillment, inventory, customer, tax, promotion, remaining
ecommerce adapters, non-`PortError` envelopes, and runtime evidence. No architecture
status is promoted from source inspection.

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
