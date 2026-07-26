# GraphQL storefront cart pricing context

Status: `source_ready_unvalidated`

This slice continues the still-open ecommerce public-error mapper cleanup without changing
FBA/FFA status.

## Closed source gap

The GraphQL storefront cart resolver creates pricing `PortContext` values with cart and line-item
correlation identity, then consumes them in `PricingReadPort` calls. Before this slice, the
returned `PortError` reached the shared public cart mapper without consumer-side access to the
original tenant, channel, actor, locale, causation, correlation, or exact pricing operation.

`safe_cart.rs` now mounts a private `ContextualPricingReadPort` through a local
`rustok_pricing` import shim. The included `cart.rs` source remains unchanged. Both pricing
constructors in that resolver now return the contextual adapter, which delegates to the canonical
`rustok_pricing::in_process_pricing_read_port`.

The adapter implements the complete current `PricingReadPort` trait:

- resolve product price;
- read one price-list projection;
- list active price-list projections;
- read admin product pricing projection;
- read storefront product pricing projection;
- preview a variant discount.

Each failed pricing owner call records `owner = "rustok_pricing"`, correlation id, tenant,
channel, locale, actor kind/id, causation id, exact operation, typed owner code/kind, owner
retryability, and the `commerce_graphql_cart` boundary before returning the same `PortError` to
the existing public mapper.

## Preserved contracts

- Existing `CART_*` GraphQL messages, codes, and retryability remain unchanged.
- Existing cart/pricing source-owner classification remains unchanged.
- Resolver mutation signatures, request construction, pricing arguments, access checks, cart
  owner calls, success responses, and layered helper routing remain unchanged.
- The adapter neither mutates pricing requests nor relabels pricing errors as cart errors.
- No owner implementation, port trait, or public transport type changes.

## Still open

- Apply equivalent original-context retention to the pricing constructor created inside the
  legacy `reprice_storefront_cart_line_items` helper path.
- Move original pre-`PortError` technical causes into correlation-aware owner-side logging where
  a pricing mapper does not already retain them.
- Execute compile, static verifier, transport, and runtime evidence before changing any
  architecture status.

## Intended verification

```bash
node scripts/verify/verify-commerce-graphql-cart-pricing-context.mjs
node scripts/verify/verify-commerce-graphql-cart-owner-context.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
cargo check -p rustok-commerce --lib
```

No verification command above was executed as part of this source wave.