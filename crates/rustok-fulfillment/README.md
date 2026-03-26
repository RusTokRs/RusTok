# rustok-fulfillment

## Purpose

`rustok-fulfillment` is the default fulfillment submodule of the `Ecommerce` family.

## Responsibilities

- Own shipping-option and fulfillment storage.
- Prepare a stable shipping boundary for checkout orchestration.
- Keep shipment lifecycle transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default fulfillment flow for the current stage, without external carrier providers.

## Interactions

- Depends on `rustok-core` for module contracts and fulfillment permission vocabulary.
- Used by `rustok-commerce` as the default fulfillment submodule of the ecommerce family.
- Links to orders and customers by identifier without taking ownership of those domains.

## Entry points

- `FulfillmentModule`
- `FulfillmentService`
- `dto::*`
- `entities::*`

See also `docs/README.md`.
