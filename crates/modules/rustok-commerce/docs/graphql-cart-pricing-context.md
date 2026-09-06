# GraphQL storefront cart pricing context

Status: `source_ready_unvalidated`

This slice continues the still-open ecommerce public-error mapper cleanup without changing
FBA/FFA status.

## Closed source gap

The GraphQL storefront cart resolver and its legacy repricing helper create pricing `PortContext`
values with cart and line-item correlation identity, then consume them in `PricingReadPort` calls.
Before these slices, returned `PortError` values reached the shared public cart mapper without
consumer-side access to the original tenant, channel, actor, locale, causation, correlation, or
exact pricing operation.

`safe_cart.rs` mounts one private `ContextualPricingReadPort` through a local `rustok_pricing`
import shim. Both pricing constructors in the included `cart.rs` resolver use that adapter.
`safe_legacy_helpers.rs` now includes the unchanged `helpers.rs` source behind a second local
pricing shim and routes its `reprice_storefront_cart_line_items` constructor to the same adapter.
The canonical owner constructor remains
`rustok_pricing::in_process_pricing_read_port` and is called only inside the shared decorator.

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
- Resolver and helper function signatures, request construction, pricing arguments, access checks,
  cart owner calls, success responses, and layered helper exports remain unchanged.
- `cart.rs` and `helpers.rs` remain included unchanged through their safe facades.
- The adapter neither mutates pricing requests nor relabels pricing errors as cart errors.
- No owner implementation, port trait, or public transport type changes.

## Still open

- Move original pre-`PortError` technical causes into correlation-aware owner-side pricing logging
  where a mapper does not already retain them.
- Review non-pricing legacy helper errors that still reach `legacy_graphql_error` only as
  `async_graphql::Error` values without typed owner context.
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
