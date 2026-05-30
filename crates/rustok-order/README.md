# rustok-order

## Purpose

`rustok-order` is the default order submodule of the `Ecommerce` family.

## Responsibilities

- Own the order write-side schema, service, and status transitions.
- Persist order snapshots and line items independently from catalog ownership.
- Persist item-level return lines in `order_return_items` with order-owned quantity and line-item validation, plus resolution links (`resolution_type`, `refund_id`, `order_change_id`) that let refund/exchange orchestration attach without moving payment logic into order storage.
- Persist `order_changes` draft/edit skeletons with preview/apply/cancel lifecycle metadata before transport orchestration is added.
- Persist typed order adjustments as language-neutral promotion/discount snapshots.
- Persist discounted order pricing as `base/compare-at` line-item prices plus
  typed `order_adjustments`, instead of collapsing sale savings into a second
  implicit price field.
- Persist first-class `shipping_total` so checkout can snapshot delivery charges
  into orders without recomputing totals from cart context later.
- Persist first-class tax-line `provider_id` so order snapshots stay stable when
  the tax engine moves beyond the default region-based provider.
- Resolve order-owned Flex attached custom fields through the shared `flex`
  multilingual attached-value contract while preserving non-Flex operational
  metadata in `orders.metadata`.
- Publish transactional order lifecycle events through the outbox.
- Publish a module-owned Leptos admin UI package in `admin/` for order
  operations and lifecycle handling.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `flex` for shared attached localized-value storage helpers used by
  order custom-field multilingual flows.
- Depends on `rustok-events` and `rustok-outbox` for transactional domain-event publishing.
- Used by `rustok-commerce` as the default order submodule of the ecommerce family.
- Keeps product and variant references as snapshots so the order domain does not depend on
  the product module as a lower-level shared layer.
- Snapshots adjustment source identity in `source_type/source_id` and keeps localized promotion display
  labels outside order-owned business storage.
- Receives checkout pricing snapshots after payment collection is calculated from
  net cart totals, so order totals do not reapply hidden discounts.
- Receives the same net total contract with `shipping_total`, keeping order
  totals aligned with payment collection and ready for later shipping-promotion
  layering.
- Receives provider-aware tax-line snapshots from checkout instead of relying on
  implicit region-only tax semantics.
- Receives shipping-scoped promotions through the same typed adjustment
  contract, so delivery discounts do not require a second implicit order total
  path.
- Exposes returns as order-owned records with optional item-level lines and resolution references while refund/exchange execution remains outside the order write model.
- Exposes order-change preview/apply/cancel service primitives as an order-owned skeleton; cross-domain transport and payment/fulfillment side effects remain outside this module.
- `apps/admin` consumes `rustok-order-admin` through manifest-driven composition,
  while GraphQL/REST order transport remains in `rustok-commerce`.

## Entry points

- `OrderModule`
- `OrderService`
- `rustok-order-admin`
- `dto::*`
- `entities::*`

See also `docs/README.md`.
