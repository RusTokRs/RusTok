# Fulfillment checkout context diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source contract records the two bounded context-validation diagnostics in
`crates/rustok-fulfillment/src/checkout_execution.rs`:

- checkout-operation causation validation in `require_operation_context`;
- tenant UUID parse-failure validation in `parse_tenant_id`.

The validators retain their existing acceptance rules, typed validation envelopes, public
call sites, and admission-before-tenant-before-causation ordering. They now emit only bounded
context, identity-shape, message-shape, and type-only diagnostic facts.

## Preserved context acceptance rules

Causation validation still:

- rejects a missing causation id;
- rejects a malformed causation id;
- rejects a parseable UUID that does not match the checkout operation id;
- accepts only the matching checkout operation UUID.

Tenant parsing still:

- accepts a valid UUID from `PortContext.tenant_id`;
- rejects an invalid UUID through the existing `map_err` path.

No acceptance rule or public operation ordering changes in this slice.

## Stable public errors

A rejected causation identity keeps the existing non-retryable validation envelope:

- code `fulfillment.checkout_operation_id_invalid`;
- message `checkout fulfillment causation_id must match the checkout operation`;
- validation kind;
- retryability `false`.

A rejected tenant identity keeps the existing non-retryable validation envelope:

- code `fulfillment.tenant_id_invalid`;
- message `PortContext.tenant_id must be a UUID for fulfillment ports`;
- validation kind;
- retryability `false`.

The exact constructed causation `PortError` is returned unchanged after diagnostics.
The exact constructed tenant `PortError` is returned unchanged after diagnostics. Neither
validator selects a replacement code, message, kind, or retryability value.

## Bounded causation diagnostic policy

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

The covered warning does not record the complete `PortError`, human-readable internal message,
raw delegated context, the expected checkout operation UUID, or Debug `PortErrorKind` output.

## Bounded tenant diagnostic policy

Tenant parse-failure diagnostics retain only type and bounded context-shape facts.

The warning records:

- the concrete UUID parse-cause type through `type_name_of_val`, without Debug or Display output;
- tenant-id character length and an explicit parse-failure fact;
- truthful owner and exact ensure/read owner operation;
- validation phase `tenant_id`;
- boundary `checkout_fulfillment_execution_port`;
- correlation id;
- stable code, closed `validation` kind label, and retryability;
- message presence and character length;
- actor-id character length and closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id, traceparent, and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds.

The tenant warning does not record the complete parse cause, the complete `PortError`, raw tenant,
actor, channel, locale, causation, traceparent, or idempotency values, human-readable internal
message text, or Debug `PortErrorKind` output.

Both context-validation paths remain ordinary `tracing::warn!` caller/delegated-context
rejections.

## Preserved behavior

This slice does not change:

- port method signatures or request/response DTOs;
- read/write admission policy or write-semantics requirements;
- admission-before-tenant-before-causation ordering;
- tenant UUID parsing or `map_err` behavior;
- causation parsing or matching semantics;
- request and immutable fulfillment-plan validation;
- fulfillment create, post-error adoption, lookup, read, or sorting behavior;
- metadata construction;
- canonical `FulfillmentError` mappings;
- Commerce orchestration;
- FBA, FFA, or ecommerce audit status.

## Static evidence

The focused guard is:

- `scripts/verify/verify-fulfillment-checkout-context-validation.mjs`.

It requires both bounded diagnostic policies, stable typed errors, exact parsing and matching
rules, both ensure/read call sites, and unchanged operation ordering. It forbids complete errors,
complete parse causes, raw context values, raw expected identity, message text, and Debug kind
output inside the covered validators.

Causation source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-causation-diagnostic-safety-source.json`.

Tenant source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-tenant-diagnostic-safety-source.json`.

Both records remain source-only: `execution` is empty and every validation flag remains false.

## Remaining diagnostic boundary

Canonical `FulfillmentError` diagnostics remain the next separate cleanup slice in
`fulfillment_error_to_port_error`.

Compile, runtime, replay, restart, contention, mounted Commerce behavior, remote-port parity,
workflows, CI, and production evidence remain open. The broad ecommerce correlation-safe mapper
cleanup and FFA/FBA status are not promoted.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
cargo check -p rustok-fulfillment --lib
```
