# Cart checkout owner tenant context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the tenant UUID rejection diagnostic gap for the four operations
published by `CartCheckoutPort` in `crates/rustok-cart/src/checkout_snapshot.rs`:

- `prepare_checkout`;
- `read_checkout_snapshot`;
- `complete_checkout`;
- `release_checkout`.

The preceding owner-admission slice retained policy and write-semantics rejection context.
After admission succeeded, all four methods still delegated to `parse_tenant_id(&context)`.
An invalid `PortContext.tenant_id` returned the stable typed validation error, but the
parser discarded the UUID cause and had no exact owner operation or cart-owned structured
diagnostics.

This slice changes only that validation boundary.

## Delivered source contract

Each public operation now passes its already selected canonical owner operation to tenant
parsing after unchanged admission:

- `PREPARE_CHECKOUT_OPERATION` for `prepare_checkout`;
- `READ_CHECKOUT_SNAPSHOT_OPERATION` for `read_checkout_snapshot`;
- `COMPLETE_CHECKOUT_OPERATION` for `complete_checkout`;
- `RELEASE_CHECKOUT_OPERATION` for `release_checkout`.

`parse_tenant_id` now accepts the retained `PortContext` and exact owner operation. When
UUID parsing fails, it:

1. captures the original UUID parse cause;
2. constructs the same `PortError::validation` value as before;
3. emits cart-owned structured warning diagnostics;
4. returns that same validation error.

Diagnostics attribute the rejection to:

- truthful owner `rustok_cart`;
- exact owner operation;
- validation phase `tenant_id`;
- boundary `cart_checkout_port`.

They retain:

- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- UUID parse cause;
- mapped validation error;
- stable internal code and message;
- typed error kind and retryability.

Tenant identity rejection uses warning severity because it is caller/context validation,
not an owner availability or invariant failure.

## Preserved behavior

This slice does not change:

- `CartCheckoutPort` method signatures or request/response DTOs;
- read/write admission policy or write-semantics requirements;
- admission-before-tenant-parsing ordering;
- public tenant validation code `cart.tenant_id_invalid`;
- public tenant validation message
  `PortContext.tenant_id must be a UUID for cart checkout`;
- validation kind or retryability;
- cart lookup, begin-checkout, context update, completion, or abandonment calls;
- active/checking-out lifecycle routing;
- checkout order metadata merge behavior;
- snapshot projection, normalization, canonical JSON, snapshot hash, or projection hash;
- delivery-group normalization;
- cart service error mapping or tax-boundary propagation;
- existing source tests;
- FBA, FFA, or ecommerce audit status.

No UUID parse cause or delegated context value is copied into the public error envelope.

## Static evidence

`scripts/verify/verify-cart-checkout-tenant-context.mjs` guards:

- four exact operation-aware parser callsites;
- admission before tenant parsing in every operation;
- one parser definition and four uses;
- retained context and exact operation parser inputs;
- UUID cause capture;
- stable validation code, message, kind, and retryability evidence;
- truthful owner, phase, boundary, and complete available context diagnostics;
- warning severity and diagnostics-before-return ordering;
- return of the same constructed validation error;
- absence of the old context-only parser signature and callsites;
- preservation of admission helpers, cart service behavior, snapshot/hash helpers,
  metadata merge, and stable public cart mapper.

The preceding `verify-cart-checkout-admission-context.mjs` guard is synchronized only to
the operation-aware tenant parser signature and retains its existing admission assertions.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- cart service error diagnostics that do not yet retain the delegated `PortContext`;
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
node scripts/verify/verify-cart-checkout-tenant-context.mjs
node scripts/verify/verify-cart-checkout-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --lib
```
