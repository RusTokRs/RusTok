# Storefront shipping enrichment owner context

Status: source-ready, unvalidated

This note records one narrow source slice in the reopened ecommerce P0 public-error safety audit. It does not promote ecommerce FBA or FFA status.

## Closed source gap

The shared storefront cart enrichment helper loads fulfillment-owned shipping options and then filters them by cart currency, public channel visibility, and shipping-profile compatibility. Before this slice, a typed `FulfillmentError` from `list_shipping_options` was immediately converted into `CommerceError::Validation(error.to_string())`.

That compatibility conversion is consumed by both the legacy GraphQL cart mutation helper and the `storefront_cart` query. Their outer safe GraphQL boundaries already return static public envelopes, but the original typed owner cause and truthful cart context were no longer available at the shared enrichment boundary.

`storefront_shipping::enrich_cart_delivery_groups` now records the typed fulfillment failure before returning the same compatibility `CommerceError` as before.

The structured event retains:

- the complete typed `FulfillmentError` cause;
- truthful `rustok_fulfillment` owner identity;
- tenant and cart identity;
- public-channel slug;
- requested and tenant-default locale inputs;
- the exact `list_shipping_options` owner operation;
- stable owner code, kind, and retryability;
- the explicit storefront shipping enrichment boundary.

Database failures are recorded at error level. Validation, missing-resource, and lifecycle rejections are recorded at warning level.

## Preserved contracts

- `enrich_cart_delivery_groups_typed` remains the canonical typed implementation.
- The compatibility helper keeps the same arguments and `CommerceResult<CartResponse>` return type.
- The compatibility helper still returns `CommerceError::Validation(error.to_string())` after diagnostics are retained.
- Cart currency filtering, public-channel visibility, shipping-profile compatibility, selected-option adoption, and successful cart projection remain unchanged.
- The legacy mutation facade still returns `Cart shipping details are temporarily unavailable` with code `CART_ENRICHMENT_UNAVAILABLE` and `retryable = true`.
- The storefront cart query remains behind the existing safe query boundary and its static fail-closed codes.
- No fulfillment entity, service, port, GraphQL type, or architecture status changes.

## Still open

- Replace the compatibility `CommerceError::Validation` bridge with a typed owner boundary once every consumer can accept the typed result directly.
- Carry request correlation, actor, causation, and deadline context into the fulfillment owner read.
- Review remaining product persistence, inventory, metadata parsing, and other non-`PortError` GraphQL helper paths.
- Execute compile, static verifier, transport, and runtime evidence before changing any architecture status.

## Intended focused checks

```bash
node scripts/verify/verify-commerce-storefront-shipping-enrichment-context.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
node scripts/verify/verify-commerce-graphql-query-error-boundary.mjs
cargo check -p rustok-commerce --lib
```

No verification command above was executed as part of this source wave.
