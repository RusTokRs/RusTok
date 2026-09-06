# Cart checkout owner admission context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the owner-admission diagnostic gap for the four operations
published by `CartCheckoutPort` in
`crates/rustok-cart/src/checkout_snapshot.rs`:

- `prepare_checkout`;
- `read_checkout_snapshot`;
- `complete_checkout`;
- `release_checkout`.

Before this slice, each operation called `PortContext::require_policy` directly and
returned admission rejection through `?`. The three write operations also called
`require_write_semantics` directly. Those failures preserved the typed `PortError`
but crossed the cart owner boundary without cart-specific owner, operation, phase,
or delegated-context diagnostics.

Consumer-side checkout inventory, payment, fulfillment, order, and compensation
context retention remains separate. This slice changes only the cart owner admission
boundary.

## Delivered source contract

The port now declares stable owner operations for all four methods and routes
admission through two cart-owned helpers:

- read admission requires `PortCallPolicy::read`;
- write admission requires `PortCallPolicy::write` and then write semantics;
- policy and write-semantics failures carry distinct admission phases;
- the original `PortError` is logged and returned unchanged.

The shared admission mapper attributes every rejection to:

- truthful owner `rustok_cart`;
- exact owner operation;
- phase `policy` or `write_semantics`;
- boundary `cart_checkout_port`.

Diagnostics retain:

- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- original internal code and message;
- typed error kind and original retryability.

Unavailable, timeout, and invariant failures use error severity. Validation,
not-found, conflict, forbidden, and other ordinary admission rejections use warning
severity.

## Preserved behavior

This slice does not change:

- `CartCheckoutPort` method signatures or request/response DTOs;
- admission ordering before tenant parsing and owner service access;
- the read/write policy selected by each operation;
- write-semantics requirements for prepare, complete, and release;
- tenant parsing or its current public validation envelope;
- cart lookup, begin-checkout, context update, completion, or abandonment calls;
- active/checking-out lifecycle routing;
- checkout order metadata merge behavior;
- snapshot projection, normalization, canonical JSON, snapshot hash, or projection
  hash behavior;
- delivery-group normalization;
- cart service error classification or public codes/messages/retryability;
- tax-boundary propagation;
- existing source tests;
- FBA, FFA, or ecommerce audit status.

No owner cause is copied into a new public envelope. Admission returns the same
`PortError` value produced by the shared `PortContext` policy helpers.

## Static evidence

`scripts/verify/verify-cart-checkout-admission-context.mjs` guards:

- stable owner, boundary, and four exact operation constants;
- one read admission helper and one write admission helper;
- policy-before-write-semantics ordering;
- explicit `policy` and `write_semantics` phases;
- mapper inputs for retained context, operation, phase, and original error;
- complete available `PortContext` and original `PortError` fields;
- technical versus ordinary rejection severity;
- exactly one read helper use and three write helper uses;
- admission before tenant parsing in every public operation;
- absence of direct context-dropping `require_policy(...)?` and
  `require_write_semantics()?` forms inside the port implementation;
- preservation of cart service calls, snapshot/hash helpers, metadata merge, public
  mapper, and existing test-source markers.

The broad ecommerce public-port-error verifier remains compatible because this slice
preserves every existing cart public error mapping and adds only internal structured
diagnostics.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- cart checkout tenant-context rejection diagnostics;
- cart service error diagnostics that do not yet retain the delegated `PortContext`;
- fulfillment execution owner admission;
- order settlement and compensation owner admission;
- inventory reservation owner admission;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and the shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-cart-checkout-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --lib
```
