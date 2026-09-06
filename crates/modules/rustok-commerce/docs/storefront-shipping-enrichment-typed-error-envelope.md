# Storefront shipping enrichment typed GraphQL error envelope

Status: source-ready / unvalidated.

## Scope

This slice continues the open ecommerce correlation-safe mapper, topology, and
non-`PortError` public-envelope work. It covers the mounted storefront GraphQL helper
that enriches a cart's delivery groups with currently available shipping options.

The broad ecommerce mapper and topology results remain open. This source change does not
claim that all GraphQL, REST, native, owner-port, compatibility, host-composition, or
remote-adapter boundaries are complete.

## Mounted cutover

`graphql/mutations/mod.rs` keeps the former cart and legacy helper implementations
private and mounts `typed_shipping_enrichment_helper.rs` through the existing layered
helper module. The cart mutation resolver continues to call `enrich_storefront_cart`
with the same arguments and receives the same `async_graphql::Result<CartResponse>`
contract.

The mounted helper now constructs a cart-scoped read `PortContext` and calls
`ShippingOptionReadPort::list_shipping_option_projections`. It no longer constructs
`FulfillmentService` or calls the compatibility conversion
`CommerceError::Validation(error.to_string())`.

The owner returns complete `ShippingOptionResponse` projections. Commerce applies the
existing currency, public-channel, shipping-profile, selected-option, and delivery-group
policies through the pure `enrich_cart_delivery_groups_from_options` function. The
existing fulfillment-service compatibility adapter delegates to the same pure function,
so its success behavior remains aligned.

## Typed owner outcomes

Every `PortErrorKind` is classified explicitly:

- validation;
- not found;
- conflict;
- forbidden;
- unavailable or timeout;
- invariant violation.

Technical unavailable, timeout, and invariant causes are retained for error-severity
diagnostics. Ordinary validation, not-found, conflict, and forbidden outcomes are
classified without adding owner messages to warning fields.

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
- owner operation `list_shipping_option_projections`;
- stable internal code, kind, and retryability;
- correlation id, tenant, actor, channel length, locale length, and deadline;
- cart UUID;
- line-item and delivery-group counts;
- character lengths for currency code, effective channel slug, requested locale, and
  tenant default locale;
- stable public code and retryability;
- boundary `commerce_graphql_storefront_shipping_enrichment`.

Only typed technical owner causes are attached to error-severity events. Ordinary owner
rejections use warning severity without a raw owner-error payload field.

The fulfillment owner port independently retains owner operation, correlation, tenant,
actor, bounded locale/channel facts, deadline, stable owner code/kind/retryability, and
the database cause only for storage failures.

## Compatibility

The former `safe_helpers.rs`, `helpers.rs`, and compatibility
`enrich_cart_delivery_groups` conversion remain private source. Their mounted helper
symbol is overridden by the typed implementation. Scoped `dead_code` allowances keep
that intentionally retained compatibility source from breaking `-Dwarnings` until a
separate retirement task has compile and upgraded-path evidence.

The service-based `enrich_cart_delivery_groups_typed` adapter remains available to
REST/native and compatibility callers. It delegates successful projection to the same
pure function but is no longer called by the mounted GraphQL helper.

## Open boundaries

This slice does not:

- inject `ShippingOptionReadPort` from the application host rather than using the root
  in-process factory;
- change REST storefront shipping enrichment;
- add native transport parity;
- remove the compatibility `CommerceError::Validation(error.to_string())` source;
- retire the fulfillment-owned service adapter;
- provide runtime, remote-profile, restart, or multi-database evidence;
- promote ecommerce or fulfillment FBA/FFA status;
- close the broad mapper, topology, and public-envelope work items.

## Verification

Suggested focused checks:

```bash
node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs
node scripts/verify/verify-commerce-graphql-shipping-enrichment-typed-error.mjs
node scripts/verify/verify-commerce-storefront-shipping-enrichment-context.mjs
cargo check -p rustok-fulfillment --lib
cargo check -p rustok-commerce --lib
```

These commands and runtime GraphQL scenarios were not executed while preparing this
source slice, per maintainer instruction. FBA and FFA status remain unchanged.
