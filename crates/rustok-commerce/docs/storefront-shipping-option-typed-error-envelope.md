# Storefront shipping-option typed GraphQL error envelope

Status: source-ready / unvalidated.

## Scope

This slice continues the open ecommerce correlation-safe mapper and non-`PortError`
public-envelope work. It covers the mounted storefront GraphQL helper that validates a
selected shipping option while a cart context is updated.

The broad ecommerce mapper result remains open. This source change does not claim that
all GraphQL, REST, native, owner-port, compatibility, or remote-adapter boundaries are
complete.

## Mounted cutover

`graphql/mutations/mod.rs` keeps the former cart and legacy helper sources private and
mounts `typed_shipping_option_helper.rs` through the existing layered helper module.
The public mutation resolver continues to call the same helper name with the same
arguments and receives the same `async_graphql::Result<()>` contract.

Only `validate_selected_shipping_option` is overridden. Cart context resolution,
selection DTOs, fulfillment reads, currency comparison, channel visibility, shipping
profile compatibility, mutation ordering, permissions, and success behavior remain in
the same order.

## Typed local outcomes

The mounted helper no longer constructs identifier-bearing `async_graphql::Error`
messages for local policy decisions. Failures are classified where they occur as:

- multiple delivery groups for the legacy single-option input;
- fulfillment owner validation;
- fulfillment owner not found;
- fulfillment owner lifecycle conflict;
- fulfillment storage unavailable;
- currency mismatch;
- public-channel unavailability;
- shipping-profile incompatibility.

Every `FulfillmentError` variant is mapped explicitly. The database variant is retained
as a typed technical cause for error-severity diagnostics. Ordinary owner and local
rejections are classified without copying their raw error text into warning fields.

## Stable public policy

Every covered outcome preserves the existing public GraphQL envelope:

| Field | Value |
| --- | --- |
| message | `Selected shipping option is invalid` |
| code | `SHIPPING_OPTION_INVALID` |
| retryable | `false` |

The selected option UUID, actual and requested currency values, channel slug, requested
locale, tenant default locale, and shipping-profile slug are not copied into the public
message.

## Diagnostics

The typed boundary records:

- truthful source owner and source operation;
- stable internal code, kind, and retryability;
- tenant, cart, and selected shipping-option UUIDs;
- selection and delivery-group counts;
- character lengths for requested and owner currency codes;
- character lengths for channel slug, requested locale, default locale, and profile
  slug;
- stable public code and retryability;
- boundary `commerce_graphql_storefront_shipping_option`.

Only the typed fulfillment database cause is attached to the technical error event.
Ordinary validation, not-found, conflict, currency, channel, profile, and request-shape
rejections use warning severity without a raw owner-error payload field.

## Compatibility

The former `safe_helpers.rs` and `helpers.rs` implementations remain private
compatibility source. Their `validate_selected_shipping_option` functions are no longer
exported through the mounted layered helper module. Scoped `dead_code` allowances keep
those intentionally retained compatibility functions from breaking `-Dwarnings` while
their retirement remains a separate task.

## Open boundaries

This slice does not:

- replace direct `FulfillmentService` construction with host-composed typed owner ports;
- change the REST storefront shipping-option boundary;
- add native transport parity;
- remove the private compatibility helpers;
- provide runtime, replay, remote-profile, or multi-database evidence;
- promote ecommerce FBA or FFA status;
- close the broad non-`PortError` envelope work item.

## Verification

Suggested focused checks:

```bash
node scripts/verify/verify-commerce-graphql-shipping-option-typed-error.mjs
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
node scripts/verify/verify-commerce-graphql-order-helper-error-safety.mjs
cargo check -p rustok-commerce --lib
```

These commands and runtime GraphQL scenarios were not executed while preparing this
source slice, per maintainer instruction. FBA and FFA status remain unchanged.
