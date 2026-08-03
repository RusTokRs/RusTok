# Fulfillment checkout causation diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice hardens only `require_operation_context` in
`crates/rustok-fulfillment/src/checkout_execution.rs`.

The validator continues to require `PortContext.causation_id` to contain a parseable UUID equal
to the checkout operation id supplied by the ensure or read request. Missing, malformed, and
mismatched causation identities remain rejected with the existing typed validation envelope.

Tenant UUID parsing in `parse_tenant_id` remains a separate open diagnostic cleanup slice.

## Preserved causation acceptance rules

The validator still:

- rejects a missing causation id;
- rejects a malformed causation id;
- rejects a parseable UUID that does not match the checkout operation id;
- accepts only the matching checkout operation UUID.

The mismatch expression is represented by `causation_id_matches_expected`, but the acceptance
semantics and public operation ordering do not change.

## Stable public error

A rejected causation identity keeps the existing non-retryable validation envelope:

- code `fulfillment.checkout_operation_id_invalid`;
- message `checkout fulfillment causation_id must match the checkout operation`;
- validation kind;
- retryability `false`.

The exact constructed causation `PortError` is returned unchanged after diagnostics. No new
public code, message, kind, or retryability value is selected.

## Bounded diagnostic policy

Causation diagnostics retain only bounded context and identity-shape facts.

The warning records:

- truthful owner `rustok_fulfillment`;
- exact owner operation `ensure_checkout_fulfillments` or `read_checkout_fulfillments`;
- validation phase `causation_id`;
- boundary `checkout_fulfillment_execution_port`;
- correlation id;
- stable code, closed `validation` kind label, and retryability;
- message presence and character length;
- tenant-id and actor-id character lengths;
- closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id presence and optional character length;
- causation UUID parse-success and expected-match facts;
- traceparent and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds;
- whether the expected checkout operation UUID is non-nil.

The covered warning does not record:

- the complete `PortError`;
- human-readable internal message text;
- raw tenant, actor, channel, locale, causation, traceparent, or idempotency values;
- the expected checkout operation UUID;
- Debug output for `PortErrorKind`.

The path remains an ordinary `tracing::warn!` validation rejection.

## Preserved behavior

This slice does not change:

- port method signatures or request/response DTOs;
- read/write admission policy or write-semantics requirements;
- admission-before-tenant-before-causation ordering;
- tenant UUID parsing;
- causation parsing or matching semantics;
- request and immutable fulfillment-plan validation;
- fulfillment create, post-error adoption, lookup, read, or sorting behavior;
- metadata construction;
- canonical `FulfillmentError` mappings;
- Commerce orchestration;
- FBA, FFA, or ecommerce audit status.

## Static evidence

The focused guard remains:

- `scripts/verify/verify-fulfillment-checkout-context-validation.mjs`.

It now requires the bounded causation diagnostic policy, the exact parsing and matching rules,
the stable error construction and return, both public call sites, and unchanged operation
ordering. It forbids complete causation errors, raw context values, raw expected identity,
message text, and Debug kind output inside the covered validator.

The same verifier intentionally requires `parse_tenant_id` to remain an explicit open residual
with its current complete error, parse cause, raw delegated context, internal message, and Debug
kind diagnostics.

Source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-causation-diagnostic-safety-source.json`.

## Remaining diagnostic boundaries

Tenant parsing remains the next separate diagnostic cleanup slice. The canonical
`FulfillmentError` mapper remains a later bounded slice.

Compile, runtime, replay, restart, contention, mounted Commerce behavior, remote-port parity,
workflows, CI, and production evidence remain open. The broad ecommerce correlation-safe
mapper cleanup and FFA/FBA status are not promoted.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
cargo check -p rustok-fulfillment --lib
```
