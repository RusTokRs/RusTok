# rustok-fulfillment

## Purpose

`rustok-fulfillment` is the default fulfillment submodule of the `Ecommerce` family.

## Responsibilities

- Own shipping-option and fulfillment storage.
- Prepare a stable shipping boundary for checkout orchestration.
- Keep shipment lifecycle transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default fulfillment flow for the current stage, without external carrier providers.
- Normalize first-class `allowed_shipping_profile_slugs` on shipping-option contracts into the temporary metadata-backed compatibility shape.
- Provide create/update/lifecycle read-side service operations for shipping-option management that the commerce facade exposes over admin REST and GraphQL.
- Publish a module-owned Leptos admin UI package in `admin/` for shipping-option operations.

## Interactions

- Depends on `rustok-core` for module contracts and fulfillment permission vocabulary.
- Used by `rustok-commerce` as the default fulfillment submodule of the ecommerce family.
- Links to orders and customers by identifier without taking ownership of those domains.
- `apps/admin` consumes `rustok-fulfillment-admin` through manifest-driven `build.rs` composition for shipping-option CRUD and lifecycle work.

## Entry points

- `FulfillmentModule`
- `FulfillmentService`
- `admin::FulfillmentAdmin` (publishable Leptos package)
- `dto::*`
- `entities::*`

See also `docs/README.md`.
