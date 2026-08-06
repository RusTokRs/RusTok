# GraphQL cart owner diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the contextual Cart owner wrapper in `safe_cart.rs`.

The wrapper surrounds eight storefront Cart port calls so the diagnostic event retains the original operation and `PortContext` when the owner returns a `PortError`. Before this change, that event emitted the complete owner error and raw tenant, actor, channel, locale, correlation, causation, traceparent, and idempotency values.

## Bounded diagnostic projection

The wrapper now projects the retained `PortContext` to closed facts:

- tenant and actor identities become `empty`, `uuid_nil`, `uuid_non_nil`, or `opaque`;
- actor kind remains `user`, `service`, or `system`;
- claims and roles become counts;
- channel, locale, correlation, causation, traceparent, and idempotency values become presence shapes;
- the deadline remains an optional millisecond value.

The diagnostic event keeps exact owner code, typed owner kind, owner retryability, and only owner-message shape and byte length. The logged error field uses `CartOwnerDiagnosticError`, whose custom `Debug` output is always `redacted`.

The original `PortError` is not consumed because the contextual wrapper must return it unchanged to the existing `cart_port_error` public mapper. It is retained only for downstream behavior and is never formatted by the owner-context event.

## Preserved behavior

This work does not change:

- any of the eight `CartStorefrontPort` delegations;
- the exact diagnostic operation selected for each delegation;
- cloning the original `PortContext` before each owner call;
- the canonical in-process Cart owner constructor;
- Cart resolver routing or mutation signatures;
- the downstream `cart_port_error` mapper;
- public `CART_*` messages, codes, and retryability;
- the previously hardened Cart/Pricing public mapper diagnostics.

The event remains error-level and retains the truthful `rustok_cart` owner and `commerce_graphql_cart` boundary.

## Verifier correction

`verify-commerce-graphql-cart-owner-context.mjs` previously required raw context fields. It now isolates the Cart owner module, requires bounded projections and safe owner facts, forbids raw context/error payloads, and preserves the eight-call delegation contract.

The verifier also follows the current precomputed Cart/Pricing `source_owner` contract in `safe_helpers.rs`.

## Remaining work

This slice does not close the complete Commerce GraphQL Cart boundary.

Still open:

- the Pricing read owner wrapper in `safe_cart.rs`;
- the Cart store-context error diagnostic in `safe_cart.rs`;
- typed line-item source and identity diagnostics;
- compatibility string classification;
- storefront shared/cart-shipping, tax, promotion, native transport, and remaining adapter cleanup.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-cart-owner-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-cart-owner-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
