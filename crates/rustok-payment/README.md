# rustok-payment

## Purpose

`rustok-payment` is the default payment submodule of the `Ecommerce` family.

## Responsibilities

- Own payment collection and payment-attempt storage.
- Prepare a stable payment boundary for checkout orchestration.
- Keep payment state transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default payment flow for the current stage, without external payment providers.

## Interactions

- Depends on `rustok-core` for module contracts and payment permission vocabulary.
- Used by `rustok-commerce` as the default payment submodule of the ecommerce family.
- Links to carts, orders, and customers by identifier without taking ownership of those domains.

## Entry points

- `PaymentModule`
- `PaymentService`
- `dto::*`
- `entities::*`

See also `docs/README.md`.
