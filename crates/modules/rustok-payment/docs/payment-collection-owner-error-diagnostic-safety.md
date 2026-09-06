# Payment collection owner-error diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice closes the canonical `payment_error_to_port_error` mapper used by
`PaymentCollectionPort` create/reuse and status operations.

The preceding source-only slices already bounded payment collection admission and tenant UUID
rejection diagnostics. This slice changes only post-delegation `PaymentError` diagnostics and the
three identifier-bearing not-found public messages.

## Bounded diagnostic policy

The mapper records only a closed variant and aggregate field shape:

- the stable owner and exact owner operation;
- correlation id and bounded `PortContext` presence/length/count facts;
- one of eleven closed `PaymentError` variant labels;
- text-field count and total character length;
- UUID-field count and non-nil count;
- opaque database-payload presence;
- stable internal code and the payment collection boundary.

It does not record the complete `PaymentError`, validation or transition text, provider identity or
operation text, database details, owner UUIDs, or raw tenant/actor/channel/locale/causation/trace/
idempotency values.

Technical failures (`ProviderUnavailable`, invalid/unknown provider outcomes, provider
configuration, and database failure) remain error severity. Validation, not-found, lifecycle
conflict, and provider rejection use warning severity.

## Public envelopes

Payment collection not-found envelopes no longer interpolate owner UUIDs:

- `payment.collection_not_found` → `payment collection was not found`;
- `payment.payment_not_found` → `payment was not found`;
- `payment.refund_not_found` → `refund was not found`.

All public codes, kinds, retryability values, and the other existing static messages remain
unchanged.

## Preserved behavior

This slice does not change:

- `PaymentCollectionPort` traits or DTOs;
- admission, tenant parsing, or their ordering;
- reusable collection lookup, create, race adoption, or status reads;
- owner service calls or snapshot conversion;
- payment provider execution, reconciliation policy, or persistence;
- checkout execution, compensation, or Commerce orchestration;
- ecommerce audit, Payment FFA, or Payment FBA status.

## Static evidence

The focused guard is:

- `scripts/verify/verify-payment-collection-owner-error-diagnostic-safety.mjs`.

Source evidence is recorded in:

- `crates/rustok-payment/contracts/evidence/payment-collection-owner-error-diagnostic-safety-source.json`.

The aggregate ecommerce guard and its negative fixture are synchronized so raw Payment collection
context or owner payloads fail closed instead of being treated as canonical.

Execution evidence remains empty. Compile, verifier, runtime, replay, restart, remote-port,
workflow, CI, and production evidence remain open. The broad ecommerce correlation-safe mapper
cleanup remains open, and no FFA/FBA status is promoted.

## Suggested maintainer checks

```bash
node scripts/verify/verify-payment-collection-owner-error-diagnostic-safety.mjs
node scripts/verify/verify-payment-collection-tenant-context.mjs
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
node --test scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs
cargo check -p rustok-payment --lib
```
