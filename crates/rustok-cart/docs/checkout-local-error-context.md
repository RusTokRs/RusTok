# Cart checkout local error context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for locally produced failures
inside `CartCheckoutPort` in `crates/rustok-cart/src/checkout_snapshot.rs`.

The preceding cart checkout slices retained:

- read/write admission rejection context;
- tenant UUID rejection context;
- `CartService` failure context.

The following local paths still returned mapped `PortError` values without retaining the
exact `PortContext` and owner operation:

- prepare-input validation;
- invalid cart status parsing before checkout admission;
- checkout lifecycle status conflict;
- snapshot projection after prepare, read, complete, and release.

Snapshot projection includes the existing snapshot hash, projection hash, normalization,
and snapshot status validation paths. This slice changes only boundary diagnostics around
those existing results.

## Delivered source contract

Every covered local result now passes through
`map_cart_checkout_local_port_error`, with:

- the retained `PortContext`;
- the exact `CartCheckoutPort` owner operation;
- a truthful local operation label;
- the already selected `PortError`.

The local operation labels are:

- `validate_prepare_input`;
- `parse_cart_status`;
- `require_checkout_status`;
- `snapshot_from_cart` for each of the four public owner operations.

The mapper emits structured diagnostics and returns the same `PortError` unchanged. It does
not construct a replacement public envelope.

Diagnostics attribute failures to:

- truthful owner `rustok_cart`;
- exact owner operation;
- exact local operation;
- boundary `cart_checkout_port`.

They retain:

- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable code and message;
- typed error kind and retryability;
- the mapped `PortError` itself.

Unavailable, timeout, and invariant failures use error severity. Validation, conflict,
not-found, forbidden, and other ordinary owner rejections use warning severity.

## Preserved behavior

This slice does not change:

- `CartCheckoutPort` method signatures or DTOs;
- admission policy or write-semantics ordering;
- tenant parsing or its validation envelope;
- any `CartService` call, argument, or ordering;
- active/checking-out lifecycle routing;
- public prepare-input validation code, message, kind, or retryability;
- public invalid-status code or current message construction;
- public checkout-status conflict code or current message construction;
- snapshot projection, normalization, canonical JSON, snapshot hash, or projection hash;
- checkout order metadata merge behavior;
- `cart_error_to_port_error` coverage and tax-boundary propagation;
- existing unit-test source;
- FBA, FFA, or ecommerce audit status.

No validator detail, raw status, projection cause, hash cause, or delegated context value is
added to a new public envelope.

## Static evidence

`scripts/verify/verify-cart-checkout-local-error-context.mjs` guards:

- prepare-input interception after the unchanged stable public mapping;
- invalid-status and checkout-status-conflict context retention;
- four snapshot-projection interception points;
- retained context, exact owner operation, exact local operation, and mapped `PortError`;
- technical-versus-ordinary severity;
- complete available delegated context diagnostics;
- diagnostics before returning the same mapped error;
- absence of the superseded context-dropping prepare paths;
- preservation of admission, tenant, service, lifecycle, snapshot/hash, metadata, public
  mapping, tax-boundary, and existing test-source markers.

The preceding admission, tenant-context, and service-error verifiers remain compatible. Their
existing source contracts are not weakened.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- fulfillment execution owner admission and validation;
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
node scripts/verify/verify-cart-checkout-local-error-context.mjs
node scripts/verify/verify-cart-checkout-service-error-context.mjs
node scripts/verify/verify-cart-checkout-admission-context.mjs
node scripts/verify/verify-cart-checkout-tenant-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --lib
```
