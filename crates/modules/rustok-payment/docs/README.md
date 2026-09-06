# `rustok-payment` Documentation

`rustok-payment` — default payment submodule of the `ecommerce` family.

## Purpose

- `payment_collections` schema;
- `payments` schema;
- `PaymentModule` and `PaymentService`;
- payment boundary for the `cart -> payment -> order` checkout chain;
- built-in manual/default payment flow at the current stage;
- payment-owned provider SPI registry for external provider composition: descriptor/adapter id validation, health/degraded-mode registration guards and side-effect-free runtime-mode checks before adapter invocation.

## Scope

- the module does not depend on `rustok-commerce` umbrella to avoid cycles;
- the module does not own the cart, order or customer profile, only references them by identifiers;
- provider-specific implementations like `stripe` are deferred to the backlog and should live as a nested submodule over the payment boundary, not mixed with the base domain model;
- GraphQL and REST transport remain in the `rustok-commerce` facade for now.

## Integration

- the module is part of the ecommerce family and must maintain its own storage/runtime boundary without returning responsibility to the umbrella `rustok-commerce`;
- transport, GraphQL and UI surfaces are published through `rustok-commerce` until a separate module-owned surface is established for the domain;
- cross-module contract changes must be synchronized with `rustok-commerce` and neighboring split modules.

## Verification

- cargo xtask module validate payment
- cargo xtask module test payment
- targeted commerce tests for the payment domain when changing runtime wiring

## Related documents

- [README crate](../README.md)
- [Commerce split plan](../../rustok-commerce/docs/implementation-plan.md)
