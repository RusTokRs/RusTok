# Fulfillment checkout local PortError diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice hardens only
`map_checkout_fulfillment_local_port_error` in
`crates/rustok-fulfillment/src/checkout_execution.rs`.

The mapper receives locally produced request, immutable-plan, identity, and set-validation
`PortError` values after their stable public code, message, kind, and retryability have already
been selected. It continues to attribute those failures to the exact public owner operation and
exact local operation.

## Bounded diagnostic policy

The local mapper records only a closed error-kind label:

- `validation`;
- `not_found`;
- `conflict`;
- `forbidden`;
- `unavailable`;
- `timeout`;
- `invariant_violation`.

It retains the stable internal code, retryability, correlation id, owner operation, local
operation, and execution boundary. Human-readable message text is represented only by
presence and character length. The complete `PortError` is not logged.

Delegated context is represented only by:

- tenant-id and actor-id character lengths;
- a closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id, traceparent, and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds.

Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are not
recorded by this mapper.

## Preserved severity and error flow

Unavailable, timeout, and invariant-violation errors continue through `tracing::error!`.
Validation, not-found, conflict, forbidden, and other ordinary rejections continue through
`tracing::warn!`.

The exact delegated `PortError` is returned unchanged. The mapper does not construct a new
code, message, kind, or retryability value.

All eight existing call sites remain in place across:

- ensure and read request validation;
- immutable fulfillment validation;
- duplicate fulfillment-set collection;
- incomplete or missing expected fulfillment-set detection;
- duplicate idempotent fulfillment-key lookup.

## Preserved behavior

This slice does not change:

- the public execution port or request/response DTOs;
- read/write admission or write-semantics ordering;
- tenant and checkout-causation validation ordering;
- fulfillment creation, post-error adoption, lookup, or read ordering;
- request and immutable-plan validation rules;
- duplicate and incomplete-set behavior;
- fulfillment sorting, metadata, or identity construction;
- canonical `FulfillmentError` to `PortError` public mapping behavior;
- Commerce orchestration;
- FFA or FBA status.

## Related admission contract

Admission diagnostics are source-ready / unvalidated under a separate contract. They now use
the same closed error-kind, message-shape, and delegated-context-shape policy while preserving
read/write admission ordering, exact phase attribution, severity, and original-error
pass-through.

The admission guard and evidence are:

- `scripts/verify/verify-fulfillment-checkout-admission-context.mjs`;
- `crates/rustok-fulfillment/contracts/evidence/checkout-admission-diagnostic-safety-source.json`;
- `crates/rustok-fulfillment/docs/checkout-admission-context.md`.

## Related context-validation contracts

Checkout causation validation and tenant UUID parse-failure diagnostics are source-ready /
unvalidated under separate bounded contracts. They preserve admission-before-tenant-before-
causation ordering, exact public call sites, stable typed errors, and original-error return.

The context guard and evidence are:

- `scripts/verify/verify-fulfillment-checkout-context-validation.mjs`;
- `crates/rustok-fulfillment/contracts/evidence/checkout-causation-diagnostic-safety-source.json`;
- `crates/rustok-fulfillment/contracts/evidence/checkout-tenant-diagnostic-safety-source.json`;
- `crates/rustok-fulfillment/docs/checkout-context-validation.md`.

Causation records only bounded context and identity-shape facts. Tenant parsing records only
parse-cause type and bounded context/message facts. Neither related contract changes the local
mapper covered here.

## Related canonical owner contract

Canonical `FulfillmentError` diagnostics are source-ready / unvalidated under a separate bounded
contract. The owner mapper preserves all five static `PortError` envelopes and the existing
warning/error severity while replacing raw causes, UUIDs, transition strings, database errors,
and delegated context with bounded facts.

The owner guard and evidence are:

- `scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs`;
- `crates/rustok-fulfillment/contracts/evidence/checkout-owner-mapper-diagnostic-safety-source.json`;
- `crates/rustok-fulfillment/docs/checkout-owner-mapper-diagnostic-safety.md`.

That contract does not change the local mapper or its eight call sites.

## Static evidence

The focused local guard is:

- `scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs`.

It requires the closed error-kind labels, message-shape and context-shape facts, both severity
paths, all eight mapper call sites, and unchanged original-error pass-through. It forbids the
complete local `PortError`, raw local context values, raw message text, and direct Debug kind
output inside the covered mapper. It also requires the separate admission boundary to remain
bounded and source-only.

Local source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-execution-local-porterror-diagnostic-safety-source.json`.

## Checkout execution source boundary

The local, admission, causation, tenant, and canonical owner contracts now make the mounted
Fulfillment checkout execution diagnostic mapper surface source-complete.

These slices are source-only. They do not claim compilation, execution, replay, restart,
contention, mounted Commerce behavior, remote-port parity, workflows, CI, or production
readiness. The broad ecommerce correlation-safe mapper cleanup remains open, and FFA/FBA status
is not promoted.

Suggested maintainer checks:

```bash
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
cargo check -p rustok-fulfillment --lib
```

These commands were intentionally not run by the implementation agent.
