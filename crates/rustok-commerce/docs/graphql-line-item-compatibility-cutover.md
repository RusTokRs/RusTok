# GraphQL line-item compatibility cutover

Status: **source-ready / unvalidated**

## Scope

This slice closes only the private storefront line-item compatibility wrappers in
`crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs`.

The layered Commerce GraphQL helper surface already selected
`typed_line_item_helpers::{resolve_storefront_line_item_input,
validate_storefront_line_item_quantity}` explicitly. The private compatibility wrappers still called
the older legacy implementations, formatted `async_graphql::Error` with `Debug`, and selected public
policy by matching English substrings.

Those two wrappers now delegate directly to the typed helper module.

## Removed compatibility policy

The removed source performed all of the following after a legacy helper failure:

- formatted the complete GraphQL error through `format!("{error:?}")`;
- searched for `Variant not found`, `Product not found`, inventory text, or metadata text;
- reconstructed `CART_PRODUCT_UNAVAILABLE`, `CART_INVENTORY_INSUFFICIENT`,
  `CART_LINE_ITEM_INVALID`, `CART_LINE_ITEM_RESOLUTION_FAILED`, or
  `CART_INVENTORY_UNAVAILABLE` envelopes;
- routed the original GraphQL error through the generic legacy diagnostic mapper.

The wrappers no longer inspect, classify, or remap errors. The typed owner boundary remains the single
policy owner for both line-item operations.

## Preserved signatures and arguments

`resolve_storefront_line_item_input` retains:

- database connection;
- tenant ID;
- `PricingReadPort`;
- `PortContext`;
- `PriceResolutionContext`;
- locale and default locale;
- public channel slug;
- complete `AddStorefrontCartLineItemInput`;
- `Result<ResolvedStorefrontLineItemInput>`.

`validate_storefront_line_item_quantity` retains:

- database connection;
- tenant ID;
- variant ID;
- requested quantity;
- public channel slug;
- `Result<()>`.

Both wrappers forward these arguments in the original order and await exactly one typed helper call.

## Preserved public policy

The typed helper continues to own these stable outcomes:

- unavailable product → `CART_PRODUCT_UNAVAILABLE`, non-retryable;
- insufficient inventory → `CART_INVENTORY_INSUFFICIENT`, non-retryable;
- invalid resolve input → `CART_LINE_ITEM_INVALID`, non-retryable;
- other resolve failures → `CART_LINE_ITEM_RESOLUTION_FAILED`, retryable;
- other quantity-validation failures → `CART_INVENTORY_UNAVAILABLE`, retryable.

The typed helper also continues to preserve Pricing and Inventory delegations, bounded diagnostic
projection, error/warn severity split, and redacted owner-source diagnostics.

## Unchanged compatibility boundaries

This slice does not change the remaining private wrappers for:

- storefront cart shipping enrichment;
- selected shipping-option validation;
- storefront cart repricing.

It also does not change customer diagnostics, cart/pricing port diagnostics, the generic legacy
GraphQL diagnostic token, typed line-item implementation details, or layered export names.

## Static guards

`verify-commerce-graphql-line-item-compatibility-cutover.mjs` checks:

- both compatibility signatures and return types;
- exact typed delegation argument order;
- one typed call per wrapper;
- absence of legacy calls, `map_err`, Debug formatting, substring matching, and local envelope
  remapping in the two wrappers;
- preservation of the three unrelated legacy helper calls;
- preservation of typed public policy, typed diagnostic redaction, and layered exports.

`verify-commerce-graphql-cart-helper-error-safety.mjs` continues to guard customer, cart-port,
legacy-diagnostic, typed mapper, and layered-routing contracts. Its compatibility count now requires
three remaining legacy calls and two typed line-item compatibility calls.

## Remaining work

Still open:

- storefront shared and cart-shipping mappers;
- tax and promotion diagnostics;
- native transports and remaining owner adapters;
- compilation, mounted execution, workflow, and CI evidence.

The broad ecommerce correlation-safe mapper cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-line-item-compatibility-cutover-source-review.json`
- `scripts/verify/verify-commerce-graphql-line-item-compatibility-cutover.mjs`
- `scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI
were run. No compile or runtime status is claimed.
