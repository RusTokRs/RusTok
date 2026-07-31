# Order checkout owner admission and context validation

Status: **source-ready / unvalidated**

## Scope

The public checkout owner wrappers preserve admission and context validation for:

- `CheckoutOrderPaymentSettlementPort`;
- `CheckoutOrderCompensationPort`.

This update changes only diagnostics. Constructors, factories, compatibility facade,
request/response contracts, validation order, and owner delegation remain unchanged.

## Public API and ordering

Both public wrappers preserve:

1. write-policy admission;
2. write-semantics admission;
3. tenant UUID parsing;
4. actor UUID parsing;
5. checkout causation matching;
6. delegation of the original context and request.

The same admission or validation `PortError` is returned unchanged.

## Safe shared context

Admission and context-rejection events retain the correlation id. Other context is
recorded only as:

- tenant and actor-id lengths;
- actor kind;
- claim and role counts;
- channel presence and length;
- locale length;
- causation, traceparent, and idempotency presence and lengths;
- deadline milliseconds.

Tenant/actor parse causes and the original mapped `PortError` remain private
structured evidence. Causation rejection records expected-operation presence/non-nil
and a false match fact, not the expected or actual UUID.

Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are
not recorded by the shared wrapper.

Unavailable, timeout, and invariant admission failures remain error severity. Other
admission and all context-validation rejections remain warning severity.

## Local settlement mapper

The payment-settlement post-delegation mapper now selects its diagnostic label from
stable `PortError.code`; public messages are not used as control flow. It records the
same safe context shape and returns the original `PortError` unchanged.

`checkout_payment_settlement.rs` is not modified by this slice. Request validation,
identity handling, lifecycle transitions, payment-reference policy, and owner error
mapping remain separate work.

## Compensation boundary

The compensation local wrapper and canonical owner use the same safe-context policy,
plus safe request, identity-comparison, lifecycle, and resource shape. Details are in
`checkout-compensation-local-context.md`.

## Preserved behavior

This slice does not change:

- public exports or module paths;
- constructor and factory parity;
- admission or context-validation envelopes;
- checkout identity reads or legacy adoption;
- settlement or compensation delegation;
- settlement owner business source;
- compensation cancellation or reconciliation behavior;
- public codes, messages, kinds, or retryability;
- Order FFA/FBA status.

## Static evidence

- `scripts/verify/verify-order-checkout-owner-context.mjs`
- `scripts/verify/verify-order-compensation-local-context.mjs`
- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json`
- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source-review.json`

The guards cover routing order, same-error return, code-only local attribution, safe
shape, absence of raw context fields, and source-only validation flags.

## Remaining gaps

Payment-settlement owner-local request/identity/lifecycle diagnostics remain a
separate Order slice. The broad ecommerce cleanup remains open for remaining owners,
adapters, non-`PortError` envelopes, and runtime evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-order-compensation-local-context.mjs
node scripts/verify/verify-order-payment-settlement-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
