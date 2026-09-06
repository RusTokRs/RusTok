# GraphQL pricing owner diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the contextual Pricing read-owner wrapper in `safe_cart.rs`.

The wrapper surrounds six `PricingReadPort` calls so the diagnostic event retains the exact operation and original `PortContext` when the owner returns a `PortError`. Before this change, that event emitted the complete owner error together with raw tenant, actor, channel, locale, correlation, and causation values.

## Bounded diagnostic projection

The Pricing wrapper now projects retained request context to closed facts:

- tenant and actor identities become `empty`, `uuid_nil`, `uuid_non_nil`, or `opaque`;
- actor kind remains `user`, `service`, or `system`;
- claims and roles become counts;
- channel, locale, correlation, causation, traceparent, and idempotency values become presence shapes;
- the optional deadline remains a millisecond value.

The event keeps exact owner code, typed owner kind, owner retryability, and only owner-message shape and byte length. Its error field uses `PricingOwnerDiagnosticError`, whose `Debug` implementation always emits `redacted`.

The original `PortError` is intentionally not consumed because the wrapper must return it unchanged to the existing `cart_port_error` mapper. It remains available only for downstream public-policy mapping and is never formatted by the Pricing owner event.

## Preserved behavior

This work does not change:

- `resolve_product_price` delegation;
- `read_price_list_projection` delegation;
- `list_active_price_list_projections` delegation;
- `read_admin_product_pricing_projection` delegation;
- `read_storefront_product_pricing_projection` delegation;
- `preview_variant_discount` delegation;
- the exact operation label and `PortContext` clone for each call;
- the canonical in-process Pricing read-port constructor;
- the downstream `cart_port_error` mapper and its source-owner classifier;
- public `CART_*` messages, codes, and retryability;
- the previously hardened Cart owner wrapper;
- resolver inclusion and shim routing.

The event remains error-level and continues to identify `rustok_pricing` and the `commerce_graphql_cart` boundary.

## Focused verifier

`verify-commerce-graphql-pricing-owner-diagnostic-safety.mjs` isolates only `pricing_read_owner_boundary` and checks:

- bounded context projection before diagnostic emission;
- redacted error formatting;
- preservation of safe owner facts;
- absence of raw context and internal owner messages;
- the six exact owner delegations and context clones;
- preservation of the canonical constructor;
- preservation of Pricing source classification and public `CART_*` envelopes in `safe_helpers.rs`.

## Remaining work

Still open:

- the Cart store-context diagnostic in `safe_cart.rs`;
- typed line-item source and identity diagnostics;
- compatibility string classification;
- storefront shared and cart-shipping mappers;
- tax, promotion, native transport, and remaining owner-adapter cleanup;
- mounted execution, compile, and runtime evidence.

The broad ecommerce correlation-safe mapper cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-pricing-owner-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-pricing-owner-diagnostic-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile or runtime status is claimed.
