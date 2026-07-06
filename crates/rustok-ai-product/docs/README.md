# `rustok-ai-product` Documentation

`rustok-ai-product` is a domain-owned support crate for product domain AI verticals.

## Purpose

- move product AI vertical ownership out of the `rustok-ai` core runtime;
- keep product-scoped AI contracts (`product_copy`, `product_attributes`) next to the product domain;
- prepare the module for phased migration of direct handler wiring.

## Area of Responsibility

- registration seam for product AI verticals;
- typed generated-payload contracts and validators for product AI tasks;
- coordination with `rustok-product`/`rustok-commerce` on read/write contracts.

## Integration

- generated payload contracts consumed by execution host: `rustok-ai`;
- domain services: `rustok-product`, `rustok-commerce`;
- operator surface: `rustok-ai` admin packages.

## Verification

- `cargo check -p rustok-ai-product`

## Related Documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
