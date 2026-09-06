# Checkout plan marketplace snapshot owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for typed
marketplace cart-line snapshot loading in
`crates/rustok-commerce/src/services/checkout_plan_builder.rs`.

The checkout plan builder already called the cart-owned
`MarketplaceCartSnapshotReadPort::list_marketplace_line_snapshots` operation.
Before this slice, the call constructed its complete `PortContext` inline and
then mapped a failed `PortError` through the generic checkout boundary mapper
with only the commerce stage. Correlation, actor, channel, locale, deadline, and
truthful owner-operation attribution were therefore unavailable at the mapper.

This slice is deliberately limited to the marketplace snapshot read performed
before typed marketplace plan-line validation. Checkout-plan product projection
reads remain a separate follow-up slice.

## Delivered source contract

The plan builder now:

- constructs one retained `marketplace_snapshot_context`;
- clones that context into
  `MarketplaceCartSnapshotReadPort::list_marketplace_line_snapshots`;
- keeps the original context available when the owner call fails;
- attributes the call to owner `rustok_cart`;
- attributes the exact owner operation `list_marketplace_line_snapshots`;
- preserves commerce stage `read_marketplace_cart_snapshots`.

Before the existing public boundary mapping runs, the marketplace-specific
mapper records:

- the original `PortError`;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key when available and the existing two-second deadline;
- owner, exact owner operation, and commerce stage;
- original code, public-safe message, typed kind, and retryability;
- boundary `commerce_checkout_plan_marketplace_snapshot`.

Unavailable, timeout, and invariant failures use error severity. Other owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- checkout request and prepared-cart identity validation;
- checking-out and non-empty cart admission;
- normalized public channel selection;
- marketplace snapshot request cart identity;
- typed marketplace snapshot-to-cart-line matching;
- legacy marketplace identity rejection when a typed snapshot is absent;
- master product, master variant, and subtotal equality checks;
- orphan snapshot rejection;
- marketplace seller projection into order line inputs;
- checkout metadata or marketplace identity stripping;
- checkout-plan product projection behavior;
- product publication, visibility, variant, and shipping-profile checks;
- inventory availability context retention and diagnostics;
- inventory request identity, quantity, or normalized channel slug;
- delivery-group and shipping-option validation;
- order adjustments, tax lines, or fulfillment plans;
- marketplace context actor, normalized/fallback locale, cart correlation
  identity, optional channel, or two-second deadline;
- `CheckoutError::BoundaryFailure` stage, kind, code, message, or retryability;
- FBA or FFA status.

The cart-owner error still maps through the generic `BoundaryFailure` envelope
with stage `read_marketplace_cart_snapshots` after diagnostics are emitted.

## Static evidence

`scripts/verify/verify-commerce-checkout-plan-marketplace-snapshot-context.mjs`
guards:

- one retained marketplace snapshot context, one delegation clone, and one
  mapper input;
- truthful cart owner and exact owner operation;
- unchanged cart snapshot request identity;
- existing actor, locale normalization/fallback, cart correlation identity,
  optional channel, and deadline construction;
- full available `PortContext` fields in diagnostics;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before unchanged public boundary mapping;
- unchanged `BoundaryFailure` fields;
- continued typed marketplace plan construction and seller projection;
- continued mounted inventory validation and context-aware inventory mapper;
- absence of the old inline context construction and context-dropping
  marketplace mapper.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- checkout-plan product and variant projection context retention;
- remaining payment execution and compensation consumers;
- remaining order and fulfillment consumers;
- inventory, customer, tax, promotion, and remaining ecommerce adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-plan-marketplace-snapshot-context.mjs
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
