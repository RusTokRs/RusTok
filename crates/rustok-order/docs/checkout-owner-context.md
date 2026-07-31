# Order checkout owner admission and context validation

Status: **source-ready / unvalidated**

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

The shared admission/context diagnostic payload is source-closed and unvalidated.
Write-admission events retain only stable code, a closed static `PortErrorKind`,
message presence/character length, retryability, and safe context shape. They do
not retain the complete `PortError`, its debug representation, or message text.

Tenant and actor parse rejections retain only the static validation phase,
`parse_failed = true`, supplied value length, stable code, static error kind, and
message shape. Checkout-causation rejection retains only causation presence/length,
static parse-failure state, expected-operation presence/non-nil state, mismatch,
and the same bounded error facts. UUID parser error payloads and complete mapped
errors are not retained.

Unavailable, timeout, and invariant admission failures remain error severity. Other
admission and all context-validation rejections remain warning severity.

## Payment-settlement boundary

The payment-settlement post-delegation mapper and canonical owner payload-diagnostic
sites are source-closed / unvalidated. The mapper retains only static
`PortErrorKind` plus message shape. The owner retains static `OrderError` variant,
aggregate text/UUID/opaque-payload shape, static parse-failure facts, and a closed
lifecycle status label. Complete errors, parser causes, owner validation text, and
transition text are not retained.

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
- settlement transition, replay, or payment-identity policy;
- compensation cancellation or reconciliation behavior;
- public codes, messages, kinds, or retryability;
- Order FFA/FBA status.

## Static evidence

- `scripts/verify/verify-order-checkout-owner-context.mjs`
- `scripts/verify/verify-order-payment-settlement-local-context.mjs`
- `scripts/verify/verify-order-payment-settlement-error-context.mjs`
- `scripts/verify/verify-order-compensation-local-context.mjs`
- `crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json`
- `crates/rustok-order/contracts/evidence/checkout-compensation-diagnostic-safety-source.json`

## Remaining gaps

No shared checkout wrapper payload-diagnostic gap remains inside this Order layer.
The broad ecommerce cleanup remains open for remaining owners, adapters,
non-`PortError` envelopes, and runtime evidence.

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
