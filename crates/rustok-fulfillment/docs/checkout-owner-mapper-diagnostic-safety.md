# Fulfillment checkout owner mapper diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice hardens only `fulfillment_error_to_port_error` in
`crates/rustok-fulfillment/src/checkout_execution.rs`.

The mapper still translates the same five `FulfillmentError` variants into the same typed
`PortError` envelopes for checkout fulfillment create, lookup, adoption, and read operations.
Only structured diagnostics change.

## Bounded diagnostic policy

Every branch retains:

- truthful owner `rustok_fulfillment`;
- exact delegated service operation;
- boundary `checkout_fulfillment_execution_port`;
- correlation id;
- one closed owner-error-kind label;
- tenant-id and actor-id character lengths;
- closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id, traceparent, and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds;
- the existing stable diagnostic code.

Variant-specific evidence is bounded as follows:

- validation retains only cause presence and character length;
- shipping-option not-found retains only whether the UUID is non-nil;
- fulfillment not-found retains only whether the UUID is non-nil;
- invalid transition retains from/to presence, character lengths, and whether the values differ;
- database failure retains only the concrete `DbErr` type through `type_name_of_val`.

The mapper does not record full validation text, raw shipping-option or fulfillment UUIDs, raw
transition values, complete database errors, or raw delegated context values.

## Preserved severity

The validation, shipping-option not-found, fulfillment not-found, and invalid-transition
branches remain `tracing::warn!` events. The database branch remains a `tracing::error!` event.

## Preserved public envelopes

The mapper keeps the exact existing constructors, codes, messages, kinds, and retryability:

- validation → `fulfillment.checkout_execution_validation` / `checkout fulfillment request is invalid`;
- shipping option not found → `fulfillment.shipping_option_not_found` / `shipping option was not found`;
- fulfillment not found → `fulfillment.fulfillment_not_found` / `fulfillment was not found`;
- invalid transition → `fulfillment.checkout_execution_state_conflict` / `fulfillment lifecycle conflicts with checkout execution`;
- database failure → `fulfillment.database_unavailable` / `fulfillment storage is temporarily unavailable`.

No branch forwards an internal cause, UUID, lifecycle value, or database error into a public
message.

## Preserved routing and behavior

This slice does not change:

- the mapper signature or `FulfillmentError` variant matching;
- create-failure mapping through `create_checkout_fulfillment`;
- read-list mapping through `list_checkout_fulfillments_for_read`;
- lookup mapping through the caller-selected service operation;
- fulfillment create, post-error adoption, lookup, read, validation, or sorting behavior;
- admission, tenant, causation, or local-`PortError` mappers;
- request/response DTOs, metadata, Commerce orchestration, FFA, or FBA status.

## Checkout execution source boundary

Together with the existing admission, tenant, causation, and local-`PortError` contracts, this
slice makes the mounted Fulfillment checkout execution diagnostic mapper surface source-complete.
This is a source-contract statement only. It does not prove compilation, runtime behavior,
replay, restart, contention, mounted Commerce parity, remote-port parity, workflows, CI, or
production readiness.

The broad ecommerce correlation-safe mapper item remains open for other owner and adapter
boundaries. No ecommerce audit, FFA, or FBA gate is promoted.

## Static evidence

The focused guard is:

- `scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs`.

It requires all five closed owner-error-kind branches, bounded context and variant facts, exact
public envelopes, four warning paths, one error path, and all three existing service mappings.
It forbids raw causes, UUIDs, transition values, database errors, and delegated context inside
the covered mapper.

Source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-owner-mapper-diagnostic-safety-source.json`.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
cargo check -p rustok-fulfillment --lib
```
