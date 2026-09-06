# `rustok-cart` Documentation

`rustok-cart` is the default cart submodule of the `ecommerce` family.

## Purpose

- schema `carts`, `cart_line_items` and `cart_line_item_translations` (localized line-item titles extracted from base rows);
- `CartModule` and `CartService`;
- persisted cart context snapshot: `region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`,
  `customer_id`, `email`, `currency_code`;
- typed `cart_adjustments` for promotion/discount snapshot: `source_type/source_id`, `amount/currency_code`,
  optional line-item binding and language-neutral metadata without display label;
- cart lifecycle: `active -> checking_out -> completed` and `active -> abandoned`;
- CRUD line items, total calculation, seller-aware delivery-group snapshot with canonical `seller_id` and locale/country snapshot normalization for storefront context;
- repricing line items on quantity change or storefront context change (region/channel), where pricing discount
  normalizes into `base/compare_at unit_price` plus pricing-owned `cart_adjustments`, so that persisted `unit_price`
  does not drift between effective sale price and base row;
- module-owned storefront package `rustok-cart/storefront` for cart inspection and safe line-item decrement/remove actions.

## Scope

- the module does not depend on the `rustok-commerce` umbrella to avoid creating a cycle;
- product/variant references in the cart are stored as snapshot references, not as mandatory cross-module foreign keys;
- cart stores a snapshot of storefront context, but does not own region/locale policy: tenant locale enablement and
  cross-module orchestration remain at the `rustok-commerce` umbrella level;
- GraphQL and REST transport still remain behind the `rustok-commerce` facade;
- storefront cart UI now lives inside the module itself and does not return cart ownership back to the host or umbrella UI.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- transport and GraphQL are still published via `rustok-commerce`, but the storefront cart read-side, seller-aware delivery-group snapshot and safe line-item write-side have already been extracted into a separate module-owned surface `rustok-cart/storefront`;
- cart delivery grouping and shipping selection use only canonical `seller_id` plus shipping profile; `seller_scope` is not read as a grouping or selection fallback.
- `cart_adjustments` are the source of truth for the discount snapshot in the cart: `subtotal_amount`, `adjustment_total`
  and net `total_amount` do not depend on default locale or localized promotion label.
- selected shipping options now materialize as first-class `shipping_total`: persisted cart total
  is calculated as `subtotal - adjustments + shipping_total (+ tax for tax-exclusive region)` instead of
  an implicit legacy shortcut via `selected_shipping_option_id`.
- typed promotion runtime now covers not only cart/line-item scope, but also shipping scope:
  shipping discounts live as `cart_adjustments` with `scope=shipping`, not as a hidden mutation of
  `shipping_total` or a separate non-snapshotting side effect.
- tax calculation is no longer hardcoded directly in `CartService`: the cart runtime now calls
  `rustok-tax::TaxCalculationPort` with a typed tenant/actor/locale/channel/deadline context, and `cart_tax_lines` carry typed `provider_id`, so that future
  external tax engines do not break the cart/order transport contract in a second migration slice.
- storefront transport parity for this layer is already confirmed: `/store/carts/{id}` and storefront
  GraphQL checkout preserve `shipping_total`, `adjustment_total` and shipping-scoped promotion
  metadata without hidden fallback or collapsing the discount into the base price.
- pricing-driven repricing only rewrites pricing-owned adjustments for affected line items and does not mix
  discount snapshot with manual/non-pricing adjustments.
- storefront add-to-cart with a discount also writes the pricing snapshot atomically: line item and pricing-owned
  adjustment are created in a single cart transaction without an intermediate persisted sale-only state.
- typed promotion runtime on top of this layer already supports preview/apply for percentage/fixed discounts
  at cart-level and line-item scope, where the application path does not require raw full-replace of all adjustments.
- aggregate admin GraphQL transport already lifts this runtime as an operator-side preview/apply path
  for `cart`, `line_item` and `shipping` scope, so cart promotions no longer live only at the
  service/test level.
- cross-module contract changes must be synchronized with `rustok-commerce` and adjacent split modules;
- storefront package uses native Leptos `#[server]` functions as the default data layer and keeps GraphQL storefront contract as a fallback.
- the cart owner publishes `guest_access_http::resolve`; HTTP hosts compose this
  adapter instead of owning or duplicating guest-cart token parsing and emission.

## Verification

- `cargo xtask module validate cart`
- `cargo xtask module test cart`
- targeted commerce tests for the cart domain when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [Development Plan for `rustok-cart`](./implementation-plan.md)
- [Commerce Split Plan](../../rustok-commerce/docs/implementation-plan.md)
