# GraphQL cart shipping option owner context

Status: source-ready, unvalidated

This note records one narrow source slice in the reopened ecommerce P0 public-error safety audit. It does not promote ecommerce FBA or FFA status.

## Closed source gap

The legacy GraphQL storefront cart helper validates selected shipping options by constructing `FulfillmentService` and calling `get_shipping_option`. The helper returns `async_graphql::Error`, so the typed `FulfillmentError` was previously converted before the safe GraphQL boundary could retain truthful fulfillment owner diagnostics.

`safe_legacy_helpers.rs` now mounts a private fulfillment import shim. The included `helpers.rs` source remains unchanged, but its single `FulfillmentService::new` call resolves to a facade that delegates to the canonical `rustok_fulfillment::FulfillmentService`.

Before returning the original typed error, the facade records:

- the complete `FulfillmentError` cause;
- truthful `rustok_fulfillment` owner identity;
- tenant and shipping-option identity;
- requested and tenant-default locale inputs;
- the exact `get_shipping_option` owner operation;
- stable owner code, kind, and retryability;
- the explicit legacy GraphQL cart helper boundary.

Database failures are recorded at error level. Validation, not-found, and lifecycle rejections are recorded at warning level. The facade returns the same `FulfillmentResult<ShippingOptionResponse>` as the canonical owner service.

## Preserved contracts

- `helpers.rs` remains included unchanged through `safe_legacy_helpers.rs`.
- Shipping-option lookup arguments and locale fallback inputs are unchanged.
- Currency, public-channel visibility, and shipping-profile compatibility checks are unchanged.
- The outer safe helper still returns `Selected shipping option is invalid` with code `SHIPPING_OPTION_INVALID` and `retryable = false`.
- No fulfillment entity, owner service, public transport type, or owner port changes.

## Still open

- Replace the legacy shipping-option service construction with a typed owner port once the owner projection carries every field required by currency, channel-visibility, and shipping-profile compatibility policy.
- Retain request correlation, actor, channel, cart, and causation context across that typed owner call.
- Review cart enrichment, product persistence, inventory, metadata parsing, and the remaining non-pricing legacy helper errors that still reach `legacy_graphql_error` as `async_graphql::Error` values.
- Execute compile, static verifier, transport, and runtime evidence before changing any architecture status.

## Intended focused checks

```bash
node scripts/verify/verify-commerce-graphql-cart-shipping-option-context.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
node scripts/verify/verify-commerce-graphql-cart-pricing-context.mjs
cargo check -p rustok-commerce --lib
```

No verification command above was executed as part of this source wave.
