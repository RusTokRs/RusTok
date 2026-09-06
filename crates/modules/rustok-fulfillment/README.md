# rustok-fulfillment

## Purpose

`rustok-fulfillment` is the default fulfillment submodule of the `Ecommerce` family.

## Responsibilities

- Own shipping-option and fulfillment storage.
- Own typed `fulfillment_items` storage inside each fulfillment.
- Track per-item `shipped_quantity` and `delivered_quantity` inside `fulfillment_items` for partial delivery progress.
- Prepare a stable shipping boundary for checkout orchestration.
- Keep shipment lifecycle transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default fulfillment flow for the current stage.
- Expose a fulfillment-owned provider SPI registry with external carrier registration validation and side-effect-free runtime-mode guardrails before adapter invocation.
- Own storefront shipping handoff and seller-aware shipping selection presentation through `rustok-fulfillment/storefront`; commerce composes it through the aggregate checkout workspace and the explicit checkout runtime API.
- Normalize first-class `allowed_shipping_profile_slugs` on shipping-option contracts into the metadata-backed compatibility shape while older stored rows are still read.
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
- `rustok-commerce-storefront` consumes `rustok-fulfillment-storefront` for delivery-group shipping selection UI while it still orchestrates cross-module checkout transport and delegates shipping-selection fallback policy to the fulfillment-owned transport facade.

## Entry points

- `FulfillmentModule`
- `FulfillmentService`
- `providers::*`
- `admin::FulfillmentAdmin` (publishable Leptos package)
- `dto::*`
- `entities::*`

See also `docs/README.md`.
