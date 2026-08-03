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
- canonical `FulfillmentError` to `PortError` mapping;
- Commerce orchestration;
- FFA or FBA status.

## Remaining diagnostic boundaries

Admission diagnostics remain a separate open cleanup slice. They still retain the complete
admission `PortError`, human-readable message text, and raw delegated context values.

Checkout causation validation, tenant parsing, and the canonical `FulfillmentError` mapper
also remain separate bounded slices. This change does not claim that the complete
`checkout_execution.rs` diagnostic surface is source-closed.

## Static evidence

The focused guard is:

- `scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs`.

It requires the closed error-kind labels, message-shape and context-shape facts, both severity
paths, all eight mapper call sites, and unchanged original-error pass-through. It forbids the
complete local `PortError`, raw local context values, raw message text, and direct Debug kind
output inside the covered mapper. It also requires the admission mapper to remain explicitly
open and out of scope.

Source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-execution-local-porterror-diagnostic-safety-source.json`.

## Evidence boundary

This slice is source-only. It does not claim compilation, execution, replay, restart,
contention, mounted Commerce behavior, remote-port parity, workflows, CI, or production
readiness. The broad ecommerce correlation-safe mapper cleanup remains open.

Suggested maintainer checks:

```bash
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
cargo check -p rustok-fulfillment --lib
```

These commands were intentionally not run by the implementation agent.
