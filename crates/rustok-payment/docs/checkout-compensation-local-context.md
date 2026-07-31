# Payment checkout compensation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source boundary covers both layers of the canonical Payment checkout compensation port:

- the public context wrapper in `checkout_compensation_context.rs`;
- the private persistent owner in `checkout_compensation.rs`;
- `CheckoutPaymentCompensationPort::compensate_checkout_payment`;
- public root and `rustok_payment::checkout_compensation::*` construction.

The public wrapper still delegates the original `PortContext` and request to the private owner. The
owner still owns write admission, tenant and causation validation, typed collection lifecycle policy,
provider cancellation, provider-journal replay/checkpointing, local cancellation, and reconciliation.

## Public wrapper

The wrapper retains its established contract:

1. capture safe context and request shape;
2. delegate the original context and request;
3. classify known returned outcomes from stable `PortError.code`;
4. return the same `PortError` unchanged.

Human-readable `PortError.message` is not used as control flow. Unknown codes pass through without an
additional wrapper event.

Wrapper diagnostics retain correlation plus lengths, counts, presence flags, actor kind, deadline,
UUID non-nil facts, reason length, metadata kind, and metadata entry count. They do not record raw
tenant, actor, channel, locale, causation, traceparent, idempotency, checkout-operation, collection,
reason, or metadata values.

Wrapper error and warning events retain only:

- stable `PortError.code`;
- a closed static `PortErrorKind` label;
- message presence and character length;
- retryability.

They do not record the complete `PortError`, debug representation, or human-readable message text. The
same original `PortError` is returned after the private event, so public code, message, kind, and
retryability are unchanged.

## Persistent owner diagnostics

The private owner uses one bounded context model for:

- invalid tenant context;
- checkout-operation causation mismatch;
- `PaymentError` owner mapping;
- manual reconciliation;
- provider cancel request/result serialization failures;
- recovered, success, failure, reconciliation, and final commit checkpoint failures;
- malformed persisted provider cancel results.

Five `PaymentError` event sites retain only:

- a stable error variant;
- text-field count and aggregate character length;
- UUID-field count and non-nil count;
- whether an opaque database payload is present.

Three codec event sites retain only static request-encoding, result-encoding, or result-decoding
failure labels. Tenant UUID parsing retains only a failure flag. Manual reconciliation retains only
whether the private reason is present and its character length.

Owner events also retain truthful owner/operation, stable code/local operation, per-call correlation,
safe context shape, and provider-journal operation presence/non-nil facts. They do not record complete
`PaymentError`, serde or UUID parser text, manual-reconciliation reason text, database payloads,
validation/provider strings, UUID values, or raw context/request/provider/financial values.

## Preserved behavior

This source cleanup does not change:

- public trait, request, response, wrapper, factory, or module exports;
- stable-code local-operation mapping;
- wrapper technical/integrity severity classification;
- unknown-code passthrough;
- write policy, deadline, idempotency, tenant, or causation admission order;
- optional missing-collection no-op behavior;
- captured-payment refund-policy reconciliation;
- `PaymentCollectionStatusKind` lifecycle routing;
- provider selection or manual-provider fallback;
- canonical `payment_collection:{collection_id}:cancel` idempotency key;
- provider request metadata and `cancel_payment_collection` marker;
- provider identity recovery from committed authorization;
- provider journal begin, claim, replay adoption, error, success, commit, or reconciliation transitions;
- provider cancellation execution;
- local cancellation race adoption;
- final provider-operation commit checkpoint;
- `PaymentError` to public `PortError` mapping;
- public error code, message, kind, or retryability;
- manual-reconciliation public conflict envelope;
- Payment FFA/FBA status.

## Source status

Together, the wrapper and persistent-owner contracts close the currently identified checkout payment
compensation payload-diagnostic sites at source level.

This is not compile, test, provider-replay, process-exit, restart, contention, mounted-transport,
remote-profile, workflow, CI, production, FFA, or FBA evidence. The broad ecommerce
correlation-safe mapper and non-`PortError` envelope item remains open across other modules.

## Static evidence

- `crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source.json`
- `crates/rustok-payment/contracts/evidence/checkout-compensation-wrapper-diagnostic-safety-source-review.json`
- `crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source.json`
- `crates/rustok-payment/contracts/evidence/checkout-compensation-owner-diagnostic-safety-source-review.json`
- `scripts/verify/verify-payment-checkout-compensation-wrapper-error-diagnostic-safety.mjs`
- `scripts/verify/verify-payment-checkout-compensation-owner-payload-diagnostic-safety.mjs`
- `scripts/verify/verify-payment-checkout-compensation-local-context.mjs`

The focused wrapper guard checks stable-code attribution, closed error-kind labels, message shape,
absence of complete `PortError` payloads, unchanged severity routing, and same-error return. The
focused owner guard checks all eight owner error sites, payment-error shape, static codec/parse facts,
manual-reconciliation reason shape, preserved journal/lifecycle markers, and source-only validation
flags. The broader compensation guard retains facade and owner-boundary coverage.

## Remaining gaps

Compile, provider replay, process-exit, restart, contention, mounted transport, remote profile,
workflow, CI, and production evidence remain unexecuted.

The broad ecommerce correlation-safe mapper item remains open for order, fulfillment, inventory,
customer, tax, promotion, remaining ecommerce adapters, and non-`PortError` public envelopes. No FBA
or FFA status is promoted from source inspection.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-checkout-compensation-wrapper-error-diagnostic-safety.mjs
node scripts/verify/verify-payment-checkout-compensation-owner-payload-diagnostic-safety.mjs
node scripts/verify/verify-payment-checkout-compensation-local-context.mjs
node scripts/verify/verify-payment-checkout-execution-local-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
