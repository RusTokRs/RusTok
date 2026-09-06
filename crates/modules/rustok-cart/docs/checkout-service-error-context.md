# Cart checkout owner service error context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for CartService failures
crossing `CartCheckoutPort` in `crates/rustok-cart/src/checkout_snapshot.rs`.

Five distinct owner-service operations are covered across six callsites:

- `get_cart` while preparing checkout;
- `begin_checkout` while preparing checkout;
- `update_context` while preparing checkout;
- `get_cart` while reading the checkout snapshot;
- `complete_cart` while completing checkout;
- `abandon_cart` while releasing checkout.

The preceding cart checkout slices retained policy/write-semantics rejection context and
tenant UUID rejection context. The service calls still mapped `CartError` directly through
`cart_error_to_port_error`, so the exact port operation, internal service operation, and
delegated `PortContext` were unavailable at the cart owner boundary.

This slice changes only those six owner-service error paths. Local prepare-input validation,
snapshot projection, normalization, canonical hashing, and other locally constructed
validation errors remain separate.

## Delivered source contract

Each covered CartService call now maps errors through
`map_cart_checkout_service_error`, passing:

- the retained `PortContext`;
- the exact `CartCheckoutPort` owner operation;
- the truthful internal CartService operation;
- the original typed `CartError`.

The mapper classifies the same public outcome selected by the existing
`cart_error_to_port_error` contract before diagnostics:

- validation -> `cart.checkout_validation`, non-retryable;
- cart not found -> `cart.not_found`, non-retryable;
- cart line item not found -> `cart.line_item_not_found`, non-retryable;
- invalid transition -> `cart.checkout_status_conflict`, non-retryable;
- database failure -> `cart.database_unavailable`, retryable;
- tax boundary -> the existing owner-provided code and retryability.

After diagnostics, the mapper passes the original `CartError` to the unchanged
`cart_error_to_port_error` function. It does not construct an alternative public envelope.

Diagnostics attribute every covered failure to:

- truthful owner `rustok_cart`;
- exact `CartCheckoutPort` operation;
- exact CartService operation;
- boundary `cart_checkout_port`.

They retain:

- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- the original typed `CartError`;
- the selected stable public code and retryability.

Database failures and tax-boundary unavailable, timeout, or invariant failures use error
severity. Validation, not-found, conflict, forbidden-like tax rejection, and other ordinary
owner rejections use warning severity.

## Preserved behavior

This slice does not change:

- `CartCheckoutPort` method signatures or request/response DTOs;
- admission policy, write-semantics requirements, or admission ordering;
- operation-aware tenant UUID parsing or its public validation envelope;
- CartService method selection, arguments, or call ordering;
- active/checking-out lifecycle routing;
- checkout order metadata merge behavior;
- `cart_error_to_port_error` match coverage;
- public codes, messages, kinds, or retryability;
- tax-boundary propagation;
- prepare-input validation mapping;
- snapshot projection, normalization, canonical JSON, snapshot hash, or projection hash;
- delivery-group normalization;
- existing source tests;
- FBA, FFA, or ecommerce audit status.

No database cause, transition detail, cart identity, line-item identity, validation text, tax
cause, or delegated context value is copied into a new public envelope.

## Static evidence

`scripts/verify/verify-cart-checkout-service-error-context.mjs` guards:

- six exact context-aware service callsites;
- five truthful CartService operation labels, including two `get_cart` callsites;
- retained context, exact owner operation, exact service operation, and original typed error;
- stable public code/retryability selection for every `CartError` variant;
- database and typed tax-boundary technical severity;
- complete available delegated context diagnostics;
- diagnostics before delegation to the unchanged public mapper;
- absence of the six superseded direct service mappings;
- preservation of the one local prepare-input direct mapping;
- preservation of admission, tenant parsing, lifecycle, metadata, snapshot/hash helpers,
  public mapping, tax-boundary propagation, and existing test-source markers.

The preceding `verify-cart-checkout-admission-context.mjs` guard is synchronized only for
the two additional owner/boundary diagnostic branches. Its admission assertions are
unchanged. The tenant-context guard requires no synchronization.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- local cart prepare-input and snapshot/projection validation diagnostics that do not yet
  retain the delegated `PortContext` and exact owner operation;
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
node scripts/verify/verify-cart-checkout-service-error-context.mjs
node scripts/verify/verify-cart-checkout-admission-context.mjs
node scripts/verify/verify-cart-checkout-tenant-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --lib
```
