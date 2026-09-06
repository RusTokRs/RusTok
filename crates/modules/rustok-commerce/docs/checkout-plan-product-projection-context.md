# Checkout plan product and variant projection owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for product and
variant projection reads in
`crates/rustok-commerce/src/services/checkout_plan_builder.rs`.

The checkout plan builder already used the typed `ProductCatalogReadPort` to load
a product either by known product identity or by variant identity. Before this
slice, both mutually exclusive branches constructed a complete `PortContext`
inline inside the owner call and then shared a generic error mapper that knew only
the commerce stage. Correlation, actor, channel, locale, deadline, and the exact
owner operation were therefore unavailable at failure mapping time.

This slice is deliberately limited to the two projection branches used while
validating cart inventory. Marketplace cart-line snapshot loading and inventory
availability context retention were closed by preceding slices.

## Delivered source contract

For every cart line that has a variant identity, the plan builder now:

- constructs one retained `product_context` before selecting the projection path;
- clones that context into `ProductCatalogReadPort::read_product_projection` when
  the cart line has a product identity;
- clones the same retained context into
  `ProductCatalogReadPort::read_variant_product_projection` otherwise;
- keeps the original context available when either mutually exclusive owner call
  fails;
- attributes both calls to owner `rustok_product`;
- attributes the exact owner operation as `read_product_projection` or
  `read_variant_product_projection`;
- preserves commerce stage `read_checkout_product_projection`.

Before the existing public boundary mapping runs, the product-specific mapper
records:

- the original `PortError`;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key when available and the existing two-second deadline;
- owner, exact owner operation, and commerce stage;
- original code, public-safe message, typed kind, and retryability;
- boundary `commerce_checkout_plan_product_projection`.

Unavailable, timeout, and invariant failures use error severity. Other owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- checkout request and prepared-cart identity validation;
- checking-out and non-empty cart admission;
- marketplace snapshot context retention, request identity, typed validation, or
  seller projection;
- selection of product-by-id versus product-by-variant projection;
- product and variant projection request identity;
- requested locale or the existing `None` fallback locale;
- cart-line handling when no variant identity exists;
- variant membership validation;
- product active, publication, and public-channel visibility checks;
- shipping-profile snapshot validation;
- inventory availability context retention and diagnostics;
- inventory request identity, quantity, or normalized channel slug;
- insufficient-inventory validation text;
- delivery-group and shipping-option validation;
- checkout metadata, order input, adjustments, tax lines, or fulfillment plans;
- product context actor, normalized/fallback locale, cart correlation identity,
  optional channel, or two-second deadline;
- `CheckoutError::BoundaryFailure` stage, kind, code, message, or retryability;
- FBA or FFA status.

Both product-owner failures still map through the generic `BoundaryFailure`
envelope with stage `read_checkout_product_projection` after diagnostics are
emitted.

## Static evidence

`scripts/verify/verify-commerce-checkout-plan-product-projection-context.mjs`
guards:

- one retained product context;
- two source branch clone sites and two mapper context inputs;
- one exact operation selector for each mutually exclusive projection branch;
- truthful product owner and exact operation attribution;
- unchanged product and variant request identity and locale behavior;
- existing actor, locale normalization/fallback, cart correlation identity,
  optional channel, and deadline construction;
- full available `PortContext` fields in diagnostics;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before unchanged public boundary mapping;
- unchanged `BoundaryFailure` fields;
- unchanged product availability, variant membership, shipping-profile, and
  inventory validation;
- continued context-aware marketplace and inventory mappers;
- absence of the old inline context construction and context-dropping shared
  projection mapper.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- remaining payment execution and compensation consumers;
- remaining order and fulfillment consumers;
- inventory, customer, tax, promotion, and remaining ecommerce adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-plan-product-projection-context.mjs
node scripts/verify/verify-commerce-checkout-plan-marketplace-snapshot-context.mjs
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
