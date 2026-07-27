# Fulfillment checkout owner admission context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the admission diagnostic gap for the two operations published by
`CheckoutFulfillmentExecutionPort` in
`crates/rustok-fulfillment/src/checkout_execution.rs`:

- `ensure_checkout_fulfillments`;
- `read_checkout_fulfillments`.

Both operations previously called `PortContext` admission methods directly. A policy or
write-semantics rejection returned the correct typed `PortError`, but the fulfillment owner
did not retain the delegated context, exact owner operation, or admission phase in structured
diagnostics.

This slice changes only that admission boundary.

## Delivered source contract

The public operations now use fulfillment-owned admission helpers:

- `ensure_checkout_fulfillments` uses write policy followed by write semantics;
- `read_checkout_fulfillments` uses read policy only.

Admission is still evaluated before tenant parsing, causation identity validation, or owner
service execution.

Each rejected admission records:

- truthful owner `rustok_fulfillment`;
- exact operation `ensure_checkout_fulfillments` or `read_checkout_fulfillments`;
- phase `policy` or `write_semantics`;
- boundary `checkout_fulfillment_execution_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- the original `PortError`;
- original code, message, kind, and retryability.

Unavailable, timeout, and invariant failures use error severity. Ordinary policy,
validation, conflict, forbidden, and other admission rejections use warning severity. The
original `PortError` is returned unchanged after diagnostics.

## Preserved behavior

This slice does not change:

- port method signatures or request/response DTOs;
- read/write policy selection;
- write-semantics requirements;
- admission-before-tenant-parsing ordering;
- tenant UUID parsing;
- checkout-operation causation validation;
- request and fulfillment validation;
- create/adopt/read service calls or ordering;
- immutable fulfillment identity checks;
- metadata construction;
- existing `FulfillmentError` public mappings;
- public codes, messages, kinds, or retryability;
- existing source verifiers and contracts;
- FBA, FFA, or ecommerce audit status.

No delegated context value or internal admission cause is copied into a new public envelope.

## Static evidence

`scripts/verify/verify-fulfillment-checkout-admission-context.mjs` guards:

- one read and one write admission helper;
- policy and write-semantics interception;
- exact operation and phase attribution;
- full available `PortContext` and original `PortError` diagnostics;
- technical-versus-ordinary severity;
- unchanged error return;
- admission before tenant parsing;
- absence of the superseded direct admission forms;
- preservation of tenant, causation, request, fulfillment, create/adopt/read, and stable
  owner-mapper behavior.

The existing `verify-fulfillment-checkout-execution-error-safety.mjs` contract remains
compatible and continues to guard tenant, causation, service-error, and public-envelope
behavior.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- fulfillment tenant UUID and checkout causation validation diagnostics;
- fulfillment request, identity, and immutable-plan validation diagnostics;
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
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
node scripts/verify/verify-fulfillment-checkout-lifecycle-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-fulfillment --lib
```
