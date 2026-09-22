# GraphQL storefront cart owner context

Status: `source_ready_unvalidated`

This slice advances the still-open ecommerce public-error mapper cleanup without changing
FBA/FFA status.

## Closed source gap

The GraphQL storefront cart resolver previously consumed a `PortContext` in each
`CartStorefrontPort` call and then passed only the returned `PortError` to the public mapper.
The public envelope stayed safe, but consumer-side diagnostics lost the original correlation,
tenant, channel, actor, causation, locale, idempotency, and exact owner-operation context.

`safe_cart.rs` now mounts a local `ContextualCartStorefrontPort` adapter through a private
`rustok_cart` import shim. The included `cart.rs` source and its success-path behavior remain
unchanged. The adapter delegates to the canonical
`rustok_cart::in_process_cart_storefront_port` and records the original `PortContext` for all
transport-neutral storefront cart operations:

- read and create storefront cart;
- add, update quantity, update pricing, and remove line item;
- update storefront context;
- reprice storefront line items.

Each failed owner call records `owner = "rustok_cart"`, correlation id, tenant, channel,
locale, actor kind/id, causation id, idempotency key, exact operation, typed owner code/kind,
owner retryability, and the `commerce_graphql_cart` boundary before returning the same
`PortError` to the existing public mapper.

## Preserved contracts

- Existing `CART_*` GraphQL messages, codes, and retryability remain unchanged.
- The resolver source, mutation signatures, request construction, access checks, service
  arguments, success responses, and layered helper routing remain unchanged.
- The adapter does not relabel pricing errors as cart errors; pricing continues through the
  existing source-owner classifier at the public boundary.
- No owner implementation, port trait, or public transport type changes.

## Still open

- Retain the original `PortContext` and exact operation for pricing calls created inside the
  GraphQL cart resolver and legacy repricing helper path.
- Move original pre-`PortError` technical causes into correlation-aware owner-side cart logging
  before conversion where they are not already retained.
- Execute compile, static verifier, transport, and runtime evidence before changing any
  architecture status.

## Intended verification

```bash
node scripts/verify/verify-commerce-graphql-cart-owner-context.mjs
node scripts/verify/verify-commerce-graphql-cart-context-error-safety.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
cargo check -p rustok-commerce --lib
```

No verification command above was executed as part of this source wave.
