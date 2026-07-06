# Documentation `rustok-order`

`rustok-order` is the default order submodule of the `ecommerce` family.

## Purpose

- schema `orders`, `order_line_items`, `order_line_item_translations` and `order_adjustments` (localized line-item titles extracted from base rows);
- `OrderModule` and `OrderService`;
- `order_returns` and `order_return_items` for an order-owned post-order returns foundation with resolution references to refund/order-change orchestration;
- `order_changes` for a draft/edit preview-apply skeleton without payment/fulfillment side effects;
- owner-owned read-helper `load_order_stats_snapshot` and DTO `OrderStatsSnapshot` for dashboard order statistics without SQL on order events inside `apps/server`;
- write-side order lifecycle: `pending -> confirmed -> paid -> shipped -> delivered/cancelled`;
- publication of order events through transactional outbox;
- module-owned admin UI package `rustok-order/admin` for order operations with split `admin/src/core/`, `admin/src/transport/mod.rs`, `admin/src/transport/graphql_adapter.rs` and `admin/src/ui/leptos.rs`.

## Responsibilities

- the module does not depend on the `rustok-commerce` umbrella to avoid creating a cycle;
- product/variant references in the order are stored as snapshot references, not as
  mandatory cross-module foreign keys;
- order line items now also carry nullable `seller_id` as a canonical multivendor snapshot key;
- order adjustments store promotion/discount snapshot as typed business data: `source_type/source_id`,
  `amount/currency_code`, optional line-item binding and metadata without localized display label;
- checkout snapshot transfers pricing repricing from cart to order so that discounted line items retain
  `base/compare_at unit_price` and savings remain in `order_adjustments`;
- GraphQL and REST transport remain in the `rustok-commerce` facade for now;
- admin UI ownership is extracted to `rustok-order/admin`;
- returns foundation stores item-level lines with quantity validation and line-item belonging to the order, while `resolution_type/refund_id/order_change_id` link a completed return to refund/exchange/claim orchestration without moving payment logic into the order boundary;
- order-change skeleton stores `preview`, `change_type`, lifecycle `pending -> applied|cancelled` and metadata, but does not yet apply cross-domain effects.
- dashboard order statistics reads `order.placed` outbox events through `rustok-order::load_order_stats_snapshot`; `apps/server` only composes the result into the root dashboard GraphQL and does not hold order SQL/DTO.

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## Integration

- the module is part of the ecommerce family and must maintain its own
  storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- transport and GraphQL are published through `rustok-commerce`, while operator UX for
  order list/detail/lifecycle is published through `rustok-order/admin`;
- checkout/create-order snapshot passes typed adjustments to `rustok-order`, and `subtotal_amount`,
  `adjustment_total` and net `total_amount` remain stable under default locale changes;
- checkout now also passes first-class `shipping_total`, so order snapshot and payment handoff
  live under a single contract `subtotal - adjustments + shipping_total (+ tax for tax-exclusive region)`;
- shipping-scoped promotions also arrive in `rustok-order` through the same typed adjustments contract,
  without a separate order-side special case for delivery discounts;
- tax snapshot is now also provider-aware: checkout transfers first-class `provider_id` into `order_tax_lines`,
  rather than hiding tax provider only in metadata;
- transport parity for this snapshot is already confirmed on storefront GraphQL checkout and admin
  order read-side: `shipping_total` and shipping-scoped adjustments reach the order contract without
  collapsing discounts into the base amount;
- payment collection before handoff to order continues to use net `cart.total_amount`, so the order snapshot
  already receives the same net pricing semantics without repeated hidden discounting;
- cross-module contract changes must be synchronized with `rustok-commerce`
  and neighboring split modules.

## FFA split for admin

The admin package now uses framework-agnostic defaults `admin/src/core/`, a facade `admin/src/transport/mod.rs` with a GraphQL adapter `admin/src/transport/graphql_adapter.rs` and an explicit Leptos render adapter `admin/src/ui/leptos.rs`; the crate root only connects the module layers and re-exports `OrderAdmin`.

## Verification

- `cargo xtask module validate order`
- `cargo xtask module test order`
- targeted commerce tests for the order domain when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)
