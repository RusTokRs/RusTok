# rustok-cart

## Purpose

`rustok-cart` is the default cart submodule of the `Ecommerce` family.

## Responsibilities

- Own the cart write-side schema and line item lifecycle.
- Persist the storefront cart context snapshot across region, locale, customer, and selected shipping intent.
- Persist typed cart adjustments as language-neutral promotion/discount snapshots.
- Persist first-class `shipping_total` so selected shipping options contribute to
  cart totals instead of remaining an implicit checkout-only side input.
- Delegate tax calculation through `rustok-tax::TaxCalculationPort` and snapshot typed tax-line
  `provider_id` instead of hardcoding region tax math directly inside the cart
  module.
- Keep cart snapshots independent from catalog ownership.
- Support repricing line items via the pricing resolver when quantity or
  storefront context changes, normalizing discounted items into
  `base/compare-at unit_price + pricing-owned adjustments` so persisted unit
  prices do not drift across channel/region boundaries.
- Prepare a stable handoff point for later checkout and order placement flows.
- Publish an owner-managed in-process `CartCheckoutPort` factory for explicit
  checkout and cart-read consumers.
- Publish an owner-managed `CartStorefrontPort` for storefront cart reads,
  mutations, and repricing without exposing `CartService` to consumers.
- Publish an owner-managed `CartPromotionPort` for cart and shipping promotion
  previews and writes without exposing promotion service methods to consumers.
- Publish a module-owned storefront cart workspace for cart inspection.
- Own the Axum guest-cart capability middleware used by HTTP hosts and owner
  transport tests.

## Interactions

- Depends on `rustok-core` for module contracts.
- Used by `rustok-commerce` as the default cart submodule of the ecommerce family.
- Keeps locale and country snapshots normalized, while letting `rustok-commerce` own cross-module
  store-context resolution against tenant locale policy and regions.
- Keeps adjustment source identity in `source_type/source_id` and never stores promotion display labels
  as cart-owned business data.
- Treats `cart_adjustments` as the discount source of truth, while pricing-driven
  repricing rewrites only pricing-owned adjustments and preserves non-pricing ones.
- Computes totals as `subtotal - adjustments + shipping_total (+ tax when the
  region is tax-exclusive)`, giving checkout and payment collection a stable
  total contract before shipping promotions are introduced.
- Keeps tax provider identity explicit in cart tax-line snapshots, so future
  external tax engines can replace the default runtime without breaking the
  downstream cart/order contract.
- Persists discounted add-to-cart snapshots atomically, so a storefront line item is
  not created in a temporary “sale-only unit price” state before pricing adjustments land.
- Exposes a typed promotion runtime for cart-level and line-item percentage/fixed
  discounts, so callers no longer need to rely on raw full-replace adjustment writes.
- Extends that typed promotion runtime to shipping scope, keeping shipping
  discounts as explicit adjustments instead of mutating `shipping_total`.
- Keeps product and variant references as snapshots so the cart domain does not depend on
  the product module as a lower-level shared layer.
- `apps/storefront` mounts `rustok-cart/storefront` via manifest-driven composition.

## Entry points

- `CartModule`
- `CartService`
- `dto::*`
- `entities::*`
- `CartView`

See also `docs/README.md`.
