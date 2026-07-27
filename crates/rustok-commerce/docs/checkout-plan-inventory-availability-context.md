# Checkout plan inventory availability owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the consumer-side structured-context gap for inventory
availability validation in
`crates/rustok-commerce/src/services/checkout_plan_builder.rs`.

The checkout plan builder already called the typed `InventoryReservationPort`
`check_availability` operation. Before this slice, each cart line constructed its
complete `PortContext` inline inside the owner call and then mapped a failed
`PortError` through the generic checkout boundary mapper with only the commerce
stage. Correlation, actor, channel, locale, deadline, and truthful owner-operation
attribution were therefore unavailable at the mapper.

This slice is deliberately limited to inventory availability checks while the
immutable checkout order plan is built. Product projection reads and marketplace
snapshot reads remain separate follow-up slices.

## Delivered source contract

For every cart line with a variant identity, the plan builder now:

- constructs one retained `inventory_context`;
- clones that context into `InventoryReservationPort::check_availability`;
- keeps the original context available when the owner call fails;
- attributes the call to owner `rustok_inventory`;
- attributes the exact owner operation `check_availability`;
- preserves commerce stage `check_inventory_availability`.

Before the existing public boundary mapping runs, the inventory-specific mapper
records:

- the original `PortError`;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key when available and the existing two-second deadline;
- owner, exact owner operation, and commerce stage;
- original code, public-safe message, typed kind, and retryability;
- boundary `commerce_checkout_plan_inventory`.

Unavailable, timeout, and invariant failures use error severity. Other owner
rejections use warning severity.

## Preserved behavior

This slice does not change:

- checkout request and prepared-cart identity validation;
- checking-out and non-empty cart admission;
- marketplace snapshot loading or typed marketplace identity validation;
- product or variant projection selection;
- product active, publication, public-channel visibility, or variant existence
  checks;
- shipping-profile snapshot validation;
- inventory request variant id, quantity, or normalized channel slug;
- insufficient-inventory validation text;
- delivery-group and shipping-option validation;
- checkout metadata, order input, adjustments, tax lines, or fulfillment plans;
- inventory context actor, normalized/fallback locale, cart correlation identity,
  optional channel, or two-second deadline;
- `CheckoutError::BoundaryFailure` stage, kind, code, message, or retryability;
- FBA or FFA status.

The inventory error still maps through the generic `BoundaryFailure` envelope with
stage `check_inventory_availability` after diagnostics are emitted.

## Static evidence

`scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs` guards:

- one retained inventory context, one delegation clone, and one mapper input;
- truthful inventory owner and exact owner operation;
- existing actor, locale normalization/fallback, cart correlation identity,
  optional channel, and deadline construction;
- unchanged inventory request identity and quantity;
- full available `PortContext` fields in diagnostics;
- original `PortError` code, message, kind, and retryability;
- typed severity and explicit boundary identity;
- diagnostics before unchanged public boundary mapping;
- unchanged `BoundaryFailure` fields;
- unchanged product availability and shipping-profile checks;
- mounted but out-of-scope marketplace snapshot mapping;
- absence of the old inline context construction and context-dropping inventory
  mapper.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- checkout plan product projection context retention;
- checkout plan marketplace snapshot context retention;
- remaining payment execution and compensation consumers;
- remaining order and fulfillment consumers;
- inventory, customer, tax, promotion, and remaining ecommerce adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
