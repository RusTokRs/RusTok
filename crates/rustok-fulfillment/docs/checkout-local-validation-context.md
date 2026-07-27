# Fulfillment checkout local validation context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for locally produced request,
set, identity, and immutable-plan failures inside `CheckoutFulfillmentExecutionPort` in
`crates/rustok-fulfillment/src/checkout_execution.rs`.

The preceding fulfillment checkout slices retained:

- read/write admission rejection context;
- tenant UUID rejection context;
- checkout causation identity rejection context;
- existing `FulfillmentError` service-operation context.

The remaining local paths still returned stable `PortError` values without retaining the
complete delegated `PortContext`, exact public owner operation, and exact local operation:

- ensure/read request validation;
- duplicate fulfillment identity while collecting a read set;
- incomplete checkout fulfillment set detection;
- missing expected fulfillment index detection;
- duplicate fulfillment identity during idempotent key lookup;
- immutable fulfillment header, item, and checkout metadata validation after ensure/read.

This slice changes only those local error boundaries.

## Delivered source contract

Every covered local result now passes through
`map_checkout_fulfillment_local_port_error`, with:

- the retained `PortContext`;
- exact public owner operation `ensure_checkout_fulfillments` or
  `read_checkout_fulfillments`;
- a truthful local operation label;
- the already selected `PortError`.

The local operation labels are:

- `validate_request`;
- `validate_fulfillment`;
- `collect_checkout_fulfillment_set`;
- `require_complete_checkout_fulfillment_set`;
- `find_checkout_fulfillment_by_key`.

The idempotent lookup helper now receives public owner operation and delegated service
operation separately. Storage failures continue to use the existing truthful service labels
`find_checkout_fulfillment_before_create` and
`adopt_checkout_fulfillment_after_create_error`, while a locally detected duplicate identity
is attributed to the public ensure operation and the exact lookup-local operation.

The mapper emits structured diagnostics and returns the same `PortError` unchanged. It does
not construct a replacement public envelope.

## Diagnostic context

Diagnostics attribute failures to:

- truthful owner `rustok_fulfillment`;
- exact public owner operation;
- exact local operation;
- boundary `checkout_fulfillment_execution_port`.

They retain:

- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable code and message;
- typed error kind and retryability;
- the mapped `PortError` itself.

Unavailable, timeout, and invariant failures use error severity. Validation, conflict,
not-found, forbidden, and other ordinary owner rejections use warning severity. All currently
covered local paths are validation or conflict outcomes and therefore remain ordinary warning
events.

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

- eight local mapper callsites and one mapper definition;
- exact ensure/read owner attribution;
- exact request, fulfillment, set, and lookup-local operations;
- public-owner versus service-operation separation in idempotent lookup;
- full available delegated context and mapped error evidence;
- technical-versus-ordinary severity;
- diagnostics before returning the same mapped error;
- all existing stable codes and messages;
- absence of the superseded context-dropping callsite shapes;
- preservation of admission, tenant, causation, service mapping, input construction, metadata,
  and service-operation behavior.

The existing admission and tenant/causation verifiers are synchronized only for the two
additional owner/boundary diagnostic branches. The execution error-safety verifier is
synchronized only for the explicit public-owner/service-operation split in `find_by_key`.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- order settlement and compensation owner admission and validation;
- inventory reservation owner admission and validation;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and the shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
node scripts/verify/verify-fulfillment-checkout-lifecycle-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-fulfillment --lib
```
