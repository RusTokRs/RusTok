# Payment checkout compensation admission context

Status: `payment_checkout_compensation_admission_context_source_unvalidated`

## Problem

The canonical payment compensation wrapper already retained correlation-safe local context for stable owner outcomes. Two gaps remained:

1. write admission failures occurred before compensation lifecycle/provider work and were deliberately excluded from the wrapper mapper;
2. the public `rustok_payment::checkout_compensation::*` namespace still exposed the persistent implementation type and factory, allowing callers to bypass the wrapper entirely.

## Source change

Payment compensation now has three explicit layers:

- private persistent owner source: `checkout_compensation.rs`;
- context wrapper: `checkout_compensation_context.rs`;
- public compatibility facade: `checkout_compensation_api.rs`.

Both the crate root and `rustok_payment::checkout_compensation::*` preserve the established public names while resolving the implementation type and factory to the context wrapper. The original trait and request DTO remain the owner contracts.

## Covered admission outcomes

The wrapper classifies only exact stable pairs:

- `port.idempotency_key_required` plus `write port calls require a non-empty idempotency key` as `admit_write_idempotency`;
- `port.deadline_required` plus `port calls require deadline semantics` as `admit_deadline`.

The first remains a validation warning. The timeout-based deadline outcome remains a technical error. The original `PortError` is returned unchanged.

## Diagnostics

Admission and covered owner outcomes retain:

- owner and public operation;
- local operation and boundary;
- correlation and tenant;
- actor, channel, locale;
- causation and traceparent;
- idempotency key and deadline;
- checkout operation and optional collection IDs;
- reason length only;
- metadata kind and entry count only;
- typed error, code, message, kind, and retryability.

Raw reason and metadata payloads are never logged.

## Preserved behavior

This slice does not change:

- write policy or write semantics;
- tenant or causation validation;
- optional no-collection success;
- collection lifecycle decisions;
- captured-payment reconciliation routing;
- provider selection, request, or idempotency key;
- provider journal recovery and checkpoints;
- local cancellation or race handling;
- public DTOs or `PortError` values;
- mounted Commerce compensation ordering.

Tenant and causation errors remain pass-through to avoid duplicate diagnostics from the persistent owner.

## Evidence boundary

The focused verifier was updated to reject:

- a public persistent compensation module;
- root or module-path exports of the persistent type/factory;
- loss of exact admission mapping;
- raw reason or metadata logging;
- changes to persistent provider/lifecycle markers.

No verifier, Cargo, test, provider replay, restart, remote-profile, mounted transport, workflow, or CI command was run. The master ecommerce mapper-cleanup item remains open for other payment surfaces and remaining ecommerce adapters.

## Suggested maintainer commands

```bash
node scripts/verify/verify-payment-checkout-compensation-local-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
