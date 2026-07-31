# Order checkout payment settlement diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source slice completes safe diagnostics for the two existing layers of
`CheckoutOrderPaymentSettlementPort`:

- the public local mapper in `checkout_owner_context.rs`;
- the canonical owner implementation in `checkout_payment_settlement.rs`.

The shared wrapper already uses stable-code-only local attribution and returns the
same delegated `PortError`. The owner implementation now records safe
context/request/identity/payment shape instead of raw identifiers, payment
references, methods, locale values, or owner-validation text.

## Shared local mapper

The wrapper recognizes these stable codes:

- `order.checkout_payment_request_invalid` → `validate_request`;
- `order.checkout_payment_identity_missing` →
  `require_durable_checkout_identity`;
- `order.checkout_payment_identity_conflict` →
  `validate_durable_checkout_identity`;
- `order.checkout_payment_state_conflict` →
  `validate_payment_settlement_lifecycle`;
- `order.checkout_payment_reference_conflict` →
  `validate_settled_payment_identity`.

Human-readable `PortError.message` is not used as control flow. Unknown codes pass
through without an added local event. The exact `PortError` is returned unchanged.

## Canonical owner diagnostics

The owner records correlation id plus safe context shape:

- tenant and actor-id lengths;
- actor kind;
- claim and role counts;
- channel presence and length;
- locale length;
- causation, traceparent, and idempotency presence and lengths;
- deadline milliseconds.

Request evidence is limited to UUID non-nil facts, payment-reference and
payment-method presence/length, and request-locale presence/length. Durable identity
evidence is limited to comparison booleans plus UUID presence/non-nil facts.
Payment replay conflicts record only reference/method equality, presence, and
length facts. Lifecycle conflicts retain the typed `OrderStatusKind`.

Database, core, and UUID parse errors remain private structured causes. Owner
validation text is reduced to presence and length. Related order resources retain
only resource kind plus identifier presence/non-nil facts.

The modified boundary does not log raw tenant, actor, channel, locale, causation,
traceparent, idempotency key, checkout operation, cart, order, payment collection,
payment reference, payment method, durable identity, or related-resource values.

## Preserved behavior

This diagnostic-only change does not alter:

- public traits, request/response DTOs, constructors, factories, or facade paths;
- write-policy and write-semantics admission;
- tenant, actor, and checkout-causation validation ordering;
- checkout identity read or explicit legacy adoption;
- acceptance of missing optional source-cart/payment-collection identity fields;
- tenant, checkout-operation, order, cart, and collection comparison rules;
- order loading or locale fallback;
- confirmed-to-paid transition and arguments passed to `mark_paid`;
- paid, shipped, or delivered replay adoption;
- pending, cancelled, or unknown lifecycle rejection;
- payment-reference and payment-method equality policy;
- `OrderError` classification;
- public codes, messages, kinds, or retryability;
- Commerce orchestration or Order FFA/FBA status.

The broad ecommerce correlation-safe mapper cleanup remains open. Compile, replay,
concurrency, restart, mounted-runtime, and remote-port evidence are not claimed.

## Static evidence

- `scripts/verify/verify-order-payment-settlement-local-context.mjs`
- `scripts/verify/verify-order-payment-settlement-error-context.mjs`
- `scripts/verify/verify-order-checkout-owner-context.mjs`
- `crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source.json`
- `crates/rustok-order/contracts/evidence/checkout-payment-settlement-diagnostic-safety-source-review.json`

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-payment-settlement-local-context.mjs
node scripts/verify/verify-order-payment-settlement-error-context.mjs
node scripts/verify/verify-order-checkout-owner-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
```
