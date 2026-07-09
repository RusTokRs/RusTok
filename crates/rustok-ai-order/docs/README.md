# `rustok-ai-order` Documentation

`rustok-ai-order` is a domain-owned support crate for order AI verticals.

## Purpose

- move order AI ownership out of the `rustok-ai` core runtime;
- keep order-scoped contracts (`order_analytics`, `order_ops_assistant`) in a separate bounded context.

## Scope

- registration seam for order AI verticals;
- typed contracts/policies for recommendation and operator-assist flows.

## Verification

- `cargo check -p rustok-ai-order`

## Related documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
