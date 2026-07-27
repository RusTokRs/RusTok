# Storefront shipping enrichment typed GraphQL error envelope

Status: source-ready / unvalidated.

## Scope

This slice continues the open ecommerce correlation-safe mapper and non-`PortError`
public-envelope work. It covers the mounted storefront GraphQL helper that enriches a
cart's delivery groups with currently available shipping options.

The broad ecommerce mapper result remains open. This source change does not claim that
all GraphQL, REST, native, owner-port, compatibility, or remote-adapter boundaries are
complete.

## Mounted cutover

`graphql/mutations/mod.rs` keeps the former cart and legacy helper implementations
private and mounts `typed_shipping_enrichment_helper.rs` through the existing layered
helper module. The cart mutation resolver continues to call `enrich_storefront_cart`
with the same arguments and receives the same `async_graphql::Result<CartResponse>`
contract.

The mounted helper now calls `enrich_cart_delivery_groups_typed` directly. It no longer
passes the typed fulfillment result through the compatibility conversion
`CommerceError::Validation(error.to_string())` or through an intermediate GraphQL error
containing owner text.

Shipping-option listing, currency filtering, public-channel visibility, shipping-profile
compatibility, selected-option adoption, delivery-group mutation, and success response
shape remain in the existing owner-provided typed implementation.

## Typed owner outcomes

Every current `FulfillmentError` variant is classified explicitly:

- validation;
- shipping option not found;
- fulfillment not found;
- lifecycle conflict;
- storage unavailable.

The fulfillment database variant is retained as a typed technical cause for
error-severity diagnostics. Ordinary validation, not-found, and lifecycle outcomes are
classified without adding their raw owner messages to warning fields.

## Stable public policy

Every covered outcome preserves the existing public GraphQL envelope:

| Field | Value |
| --- | --- |
| message | `Cart shipping details are temporarily unavailable` |
| code | `CART_ENRICHMENT_UNAVAILABLE` |
| retryable | `true` |

The owner validation text, database cause, channel slug, locale values, currency code,
shipping option details, and cart projections are not copied into the public message.

## Diagnostics

The typed boundary records:

- truthful owner `rustok_fulfillment`;
- owner operation `list_shipping_options`;
- stable internal code, kind, and retryability;
- tenant and cart UUIDs;
- line-item and delivery-group counts;
- character lengths for currency code, effective channel slug, requested locale, and
  tenant default locale;
- stable public code and retryability;
- boundary `commerce_graphql_storefront_shipping_enrichment`.

Only the typed fulfillment database cause is attached to the technical error event.
Ordinary owner rejections use warning severity without a raw owner-error payload field.

## Compatibility

The former `safe_helpers.rs`, `helpers.rs`, and compatibility
`enrich_cart_delivery_groups` conversion remain private source. Their mounted helper
symbol is overridden by the typed implementation. Scoped `dead_code` allowances keep
that intentionally retained compatibility source from breaking `-Dwarnings` until a
separate retirement task has compile and upgraded-path evidence.

## Open boundaries

This slice does not:

- replace direct `FulfillmentService` construction inside the typed owner adapter with
  host-composed ports;
- change REST storefront shipping enrichment;
- add native transport parity;
- remove the compatibility `CommerceError::Validation(error.to_string())` source;
- provide runtime, remote-profile, restart, or multi-database evidence;
- promote ecommerce FBA or FFA status;
- close the broad mapper and public-envelope work item.

## Verification

Suggested focused checks:

```bash
node scripts/verify/verify-commerce-graphql-shipping-enrichment-typed-error.mjs
node scripts/verify/verify-commerce-storefront-shipping-enrichment-context.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
cargo check -p rustok-commerce --lib
```

These commands and runtime GraphQL scenarios were not executed while preparing this
source slice, per maintainer instruction. FBA and FFA status remain unchanged.
