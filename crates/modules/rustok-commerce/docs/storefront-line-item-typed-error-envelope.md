# Storefront line-item typed GraphQL error envelope

Status: source-ready / unvalidated.

## Scope

This source slice continues the open correlation-safe mapper and non-`PortError`
public-envelope work in the canonical ecommerce implementation plan. It covers the
mounted storefront GraphQL add-line-item resolver and quantity validator.

The broad ecommerce mapper result remains open. This slice does not claim that all
commerce GraphQL, REST, native, owner-port, compatibility, or remote-adapter error
boundaries are complete.

## Mounted cutover

`graphql/mutations/mod.rs` keeps the established cart and order safe helper sources
private and mounts `layered_order_helpers.rs` as the public mutation-helper module.
The layered module preserves the existing order/cart helper symbol set and explicitly
overrides only:

- `resolve_storefront_line_item_input`;
- `validate_storefront_line_item_quantity`.

Both overrides are implemented in `typed_line_item_helpers.rs`. Existing GraphQL
field signatures, permissions, request DTOs, cart-owner calls, pricing-owner calls,
success responses, and mutation ordering are unchanged.

## Typed local outcomes

The mounted resolver no longer renders an `async_graphql::Error` with `Debug` and
searches English message fragments to decide its public response. Failures are
classified where they occur as one of:

- `ProductUnavailable`;
- `InventoryInsufficient`;
- `InputInvalid`;
- `DependencyUnavailable`.

The retained source cause is also typed as database, pricing `PortError`, inventory
`CommerceError`, metadata JSON, or a local policy rejection. Diagnostics therefore
retain truthful source owner and operation without using public message text as a
control-flow protocol.

## Stable public policy

The mounted boundary preserves the previous public policies:

| Outcome | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| product unavailable | `CART_PRODUCT_UNAVAILABLE` | `Product is not available` | `false` |
| inventory insufficient | `CART_INVENTORY_INSUFFICIENT` | `Requested quantity is not available` | `false` |
| invalid add-line-item metadata | `CART_LINE_ITEM_INVALID` | `Cart line item input is invalid` | `false` |
| unresolved add-line-item dependency | `CART_LINE_ITEM_RESOLUTION_FAILED` | `Cart line item could not be resolved` | `true` |
| unresolved quantity dependency | `CART_INVENTORY_UNAVAILABLE` | `Inventory availability could not be verified` | `true` |

No database text, pricing message, inventory validation text, variant fallback title,
SKU, metadata payload, locale value, or public channel slug is copied into the public
GraphQL message.

## Diagnostics

The boundary records:

- typed source cause kind;
- truthful source owner and source operation;
- consumer operation;
- optional pricing correlation id;
- tenant, variant, and known product ids;
- requested quantity;
- channel-slug and locale character lengths only;
- stable public code and retryability;
- boundary `commerce_graphql_storefront_line_item`.

Dependency failures use error severity. Product, inventory, and input rejections use
warning severity. Raw line-item metadata, SKU, title, channel slug, and locale are not
logged as dedicated fields.

## Compatibility

The former `safe_helpers.rs` line-item wrappers remain private compatibility source.
They are no longer exported through the mounted `helpers` module for the two covered
operations. All unrelated safe cart/order helpers continue through the existing
facades.

## Verification

Suggested source checks:

```bash
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
node scripts/verify/verify-commerce-graphql-order-helper-error-safety.mjs
cargo check -p rustok-commerce --lib
```

These checks and runtime GraphQL scenarios were not executed while preparing this
source slice. FBA and FFA status remain unchanged.
