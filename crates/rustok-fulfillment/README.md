# rustok-fulfillment

## Purpose

`rustok-fulfillment` is the default fulfillment submodule of the `Ecommerce` family.

## Responsibilities

- Own shipping-option and fulfillment storage.
- Own typed `fulfillment_items` storage inside each fulfillment.
- Track per-item `shipped_quantity` and `delivered_quantity` inside `fulfillment_items` for partial delivery progress.
- Prepare a stable shipping boundary for checkout orchestration.
- Keep shipment lifecycle transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default fulfillment flow for the current stage, without external carrier providers.
- Own storefront shipping handoff and seller-aware shipping selection presentation through `rustok-fulfillment/storefront`; commerce may still provide the transitional aggregate checkout transport callback until the fulfillment-owned transport cutover lands.
- Normalize first-class `allowed_shipping_profile_slugs` on shipping-option contracts into the temporary metadata-backed compatibility shape.
- Provide create/update/lifecycle read-side service operations for shipping-option management that the commerce facade exposes over admin REST and GraphQL.
- Return typed fulfillment items from `FulfillmentResponse` instead of forcing post-order flows to reconstruct line-item scope from metadata blobs alone.
- Support partial `ship` / `deliver` adjustments on typed fulfillment items and append language-agnostic audit events to fulfillment/item metadata while keeping `delivered_note` as a typed field.
- Support explicit `reopen` / `reship` recovery flows on top of typed fulfillment items, so delivered or cancelled fulfillments can return to actionable post-order states without language-dependent metadata hacks.
- Support post-order follow-up fulfillments through the commerce facade, where manual create paths validate order-line ownership and remaining quantities before calling `FulfillmentService`.
- Publish a module-owned Leptos admin UI package in `admin/` for shipping-option operations.

## Interactions

- Depends on `rustok-core` for module contracts and fulfillment permission vocabulary.
- Used by `rustok-commerce` as the default fulfillment submodule of the ecommerce family.
- Links to orders and customers by identifier without taking ownership of those domains.
- `apps/admin` consumes `rustok-fulfillment-admin` through manifest-driven `build.rs` composition for shipping-option CRUD and lifecycle work.
- `rustok-commerce-storefront` consumes `rustok-fulfillment-storefront` for delivery-group shipping selection UI while it still orchestrates cross-module checkout transport.

## Entry points

- `FulfillmentModule`
- `FulfillmentService`
- `admin::FulfillmentAdmin` (publishable Leptos package)
- `dto::*`
- `entities::*`

See also `docs/README.md`.
