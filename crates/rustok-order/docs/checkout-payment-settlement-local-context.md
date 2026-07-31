# Order checkout payment settlement diagnostic safety

Status: **wrapper and owner source-closed / shared admission open / unvalidated**

## Scope

This source slice completes the currently identified payload-diagnostic cleanup for
the two payment-settlement layers:

- the post-delegation mapper in `checkout_owner_context.rs`;
- the canonical owner in `checkout_payment_settlement.rs`.

The public trait, request/response DTOs, constructors, factories, admission order,
owner delegation, lifecycle behavior, and Commerce composition remain unchanged.

## Local settlement mapper

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
through without an added local event. The exact delegated `PortError` is returned
unchanged.

Both local severity branches retain only stable code, a closed static
`PortErrorKind`, message presence/character length, retryability, correlation id,
and safe context shape. They do not record the complete `PortError`, its debug
representation, or message text.

## Canonical owner diagnostics

Owner tenant and actor parsing retain only the static field label, input length,
and `parse_failed = true`; UUID parser errors are not recorded.

All seven `OrderError` variants are classified by a closed static label. Diagnostic
evidence is limited to aggregate facts:

- text-field count and total character length;
- UUID-field count and non-nil count;
- opaque-payload presence;
- optional static resource kind plus identifier presence/non-nil facts.

Database and core payloads are not formatted. Validation and invalid-transition
text is not recorded. Lifecycle rejection uses a closed seven-value
`OrderStatusKind` label instead of debug formatting.

Existing request, durable-identity, payment-reference, and payment-method events
continue to retain only comparison, presence, length, and UUID non-nil facts.

## Preserved behavior

This diagnostic-only change does not alter:

- write-policy and write-semantics admission;
- tenant, actor, or checkout-causation validation ordering;
- checkout identity read or explicit legacy adoption;
- durable identity comparison rules;
- order loading or locale fallback;
- confirmed-to-paid transition and `mark_paid` arguments;
- paid, shipped, or delivered replay adoption;
- pending, cancelled, or unknown lifecycle rejection;
- payment-reference and payment-method equality policy;
- `OrderError` classification;
- public codes, messages, kinds, or retryability;
- Commerce orchestration or Order FFA/FBA status.

## Source status and remaining gaps

The currently identified payment-settlement post-delegation mapper and canonical
owner payload sites are **source-closed / unvalidated**.

Shared checkout admission/context events still retain complete `PortError` and UUID
parse-cause payloads. That shared layer affects both settlement and compensation and
remains a separate bounded Order slice.

The broad ecommerce correlation-safe mapper cleanup remains open. Compile, replay,
concurrency, restart, mounted-runtime, remote-port, workflow, and CI evidence are
not claimed.

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
