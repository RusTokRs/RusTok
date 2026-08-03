# Fulfillment checkout local validation context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context attribution gap for locally produced request,
set, identity, and immutable-plan failures inside `CheckoutFulfillmentExecutionPort` in
`crates/rustok-fulfillment/src/checkout_execution.rs`.

The covered local paths are:

- ensure/read request validation;
- duplicate fulfillment identity while collecting a read set;
- incomplete checkout fulfillment set detection;
- missing expected fulfillment index detection;
- duplicate fulfillment identity during idempotent key lookup;
- immutable fulfillment header, item, and checkout metadata validation after ensure/read.

Every covered result passes through `map_checkout_fulfillment_local_port_error` with the
retained `PortContext`, exact public owner operation, truthful local operation, and the already
selected `PortError`.

## Preserved owner and local attribution

The public owner operations remain:

- `ensure_checkout_fulfillments`;
- `read_checkout_fulfillments`.

The local operation labels remain:

- `validate_request`;
- `validate_fulfillment`;
- `collect_checkout_fulfillment_set`;
- `require_complete_checkout_fulfillment_set`;
- `find_checkout_fulfillment_by_key`.

The idempotent lookup helper continues to receive public owner operation and delegated service
operation separately. Storage failures retain the service labels
`find_checkout_fulfillment_before_create` and
`adopt_checkout_fulfillment_after_create_error`, while a locally detected duplicate identity
is attributed to the public ensure operation and exact lookup-local operation.

## Bounded diagnostic context

The mapper emits structured diagnostics and returns the same `PortError` unchanged. It does
not construct a replacement public envelope and does not log the complete `PortError`.

Diagnostics retain:

- truthful owner `rustok_fulfillment`;
- exact public owner operation;
- exact local operation;
- boundary `checkout_fulfillment_execution_port`;
- correlation id;
- stable code and retryability;
- one closed `PortErrorKind` label;
- message presence and character length;
- tenant-id and actor-id character lengths;
- closed actor-kind label;
- claim and role counts;
- channel presence and optional length;
- locale length;
- causation-id, traceparent, and idempotency-key presence plus optional lengths;
- optional deadline milliseconds.

The mapper does not record raw tenant, actor, channel, locale, causation, traceparent, or
idempotency values. Human-readable `PortError.message` text and complete Debug output are not
recorded.

Unavailable, timeout, and invariant failures use error severity. Validation, conflict,
not-found, forbidden, and other ordinary owner rejections use warning severity. The currently
covered local paths remain validation or conflict outcomes and therefore remain ordinary
warning events.

## Preserved public envelopes

Request validation keeps the existing envelopes:

- `fulfillment.checkout_identity_invalid`;
- `fulfillment.checkout_plan_hash_invalid`;
- `fulfillment.checkout_plan_invalid`;
- `fulfillment.checkout_item_invalid`.

Set and identity validation keeps:

- `fulfillment.checkout_identity_duplicate`;
- `fulfillment.checkout_set_incomplete`.

Immutable fulfillment validation keeps:

- `fulfillment.checkout_plan_conflict`;
- `fulfillment.checkout_items_conflict`;
- `fulfillment.checkout_identity_missing`;
- `fulfillment.checkout_identity_conflict`.

Messages, kinds, and retryability remain unchanged.

## Preserved behavior

This slice does not change:

- port method signatures or request/response DTOs;
- read/write policy or write-semantics admission;
- admission, tenant, and causation validation ordering;
- request acceptance rules;
- expected fulfillment-set cardinality;
- duplicate or missing identity detection;
- immutable plan comparison rules;
- create, adoption, lookup, or read service ordering;
- stable `FulfillmentError` mapping;
- fulfillment or item metadata construction;
- fulfillment sorting;
- FBA, FFA, or ecommerce audit status.

No request value, fulfillment identity, set cardinality, stored metadata, internal key, or
delegated context value is copied into a new public envelope.

## Static evidence

`scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs` guards:

- eight local mapper call sites and one mapper definition;
- exact ensure/read owner attribution;
- exact request, fulfillment, set, and lookup-local operations;
- public-owner versus service-operation separation in idempotent lookup;
- bounded delegated context and mapped-error evidence;
- technical-versus-ordinary severity;
- diagnostics before returning the same mapped error;
- all existing stable codes and messages;
- absence of superseded context-dropping call-site shapes;
- preservation of admission, tenant, causation, service mapping, input construction, metadata,
  and service-operation behavior.

The dedicated payload-safety guard is:

- `scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs`.

Its source evidence and detailed policy are recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-execution-local-porterror-diagnostic-safety-source.json`;
- `crates/rustok-fulfillment/docs/checkout-execution-local-porterror-diagnostic-safety.md`.

## Remaining gaps

Admission diagnostics remain a separate open cleanup slice and still retain complete
`PortError`, message text, and raw delegated context. Causation validation, tenant parsing,
and canonical `FulfillmentError` diagnostics remain separate bounded slices.

Compile, runtime, replay, restart, remote-port, workflow, and CI evidence remain open. The
broad ecommerce correlation-safe mapper cleanup and FFA/FBA status are not promoted.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
cargo check -p rustok-fulfillment --lib
```
