# Order checkout owner admission and context validation

Status: **partial source-ready / unvalidated**

## Scope

The public checkout owner wrappers preserve admission and context validation for:

- `CheckoutOrderPaymentSettlementPort`;
- `CheckoutOrderCompensationPort`.

Constructors, factories, compatibility facade, request/response contracts,
validation order, and owner delegation remain unchanged.

## Public API and ordering

Both public wrappers preserve:

1. write-policy admission;
2. write-semantics admission;
3. tenant UUID parsing;
4. actor UUID parsing;
5. checkout causation matching;
6. delegation of the original context and request.

The same admission, validation, or delegated `PortError` is returned unchanged.

## Shared context shape

Events retain the correlation id. Other `PortContext` values are represented by
lengths, presence flags, actor kind, claim/role counts, and deadline milliseconds;
raw context values are not recorded.

The shared admission/context diagnostic payload itself is not yet closed: admission
branches still retain the complete `PortError`, and context rejection still retains
the mapped `PortError` plus UUID parse-cause payload. That work remains a separate
bounded source slice affecting both public operations.

Unavailable, timeout, and invariant admission failures remain error severity. Other
admission and all context-validation rejections remain warning severity.

## Local settlement mapper

The payment-settlement post-delegation mapper selects its diagnostic label from
stable `PortError.code`; public messages are not used as control flow. Both severity
branches now retain only static `PortErrorKind`, message presence/length, retryability,
correlation id, and safe context shape. They do not retain the complete error or
message text, and return the original `PortError` unchanged.

Canonical payment-settlement owner payload diagnostics remain a separate open slice.
Details are in `checkout-payment-settlement-local-context.md`.

## Compensation boundary

The compensation local wrapper and canonical owner payload-diagnostic sites are
source-closed / unvalidated. Details are in
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
- `scripts/verify/verify-order-payment-settlement-local-context.mjs`
- `scripts/verify/verify-order-compensation-local-context.mjs`
- `crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json`
- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json`

## Remaining gaps

Shared admission/context payload diagnostics and canonical payment-settlement owner
payload diagnostics remain open. The broad ecommerce cleanup remains open for
remaining owners, adapters, non-`PortError` envelopes, and runtime evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-order-payment-settlement-local-context.mjs
node scripts/verify/verify-order-payment-settlement-error-context.mjs
node scripts/verify/verify-order-compensation-local-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
