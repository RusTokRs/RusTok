# rustok-payment

## Purpose

`rustok-payment` is the default payment submodule of the `Ecommerce` family.

## Responsibilities

- Own payment collection and payment-attempt storage.
- Own refund record storage and basic refund lifecycle for the default manual flow.
- Own tenant-scoped provider-operation journals; operation reads and lifecycle mutations require the canonical tenant identity, and collection/refund ownership is enforced before journaling.
- Prepare a stable payment boundary for checkout orchestration.
- Keep payment state transitions isolated from the ecommerce umbrella.
- Provide a built-in manual/default payment flow for the current stage.
- Expose a payment-owned provider SPI registry with external registration validation and side-effect-free runtime-mode guardrails before adapter invocation.
- Bound provider operation request/result payloads, provider references, and idempotency keys at the SPI boundary before external execution or durable journal persistence.
- Keep public webhook delivery-id and replay/idempotency hints independent; verified provider output remains authoritative for both identities. Provider-event admin authentication failures are 401, while authenticated permission denials are 403.

## Interactions

- Depends on `rustok-core` for module contracts and payment permission vocabulary.
- Used by `rustok-commerce` as the default payment submodule of the ecommerce family.
- Links to carts, orders, and customers by identifier without taking ownership of those domains.

## Entry points

- `PaymentModule`
- `PaymentService`
- `providers::*`
- `dto::*`
- `entities::*`

See also `docs/README.md`.
