# `rustok-marketplace-seller` Documentation

## Purpose

`rustok-marketplace-seller` owns seller management, membership, and store bindings within the RusToK Marketplace family.

## Scope

- Domain models and storage for seller management, membership, and store bindings;
- Idempotent command ports and typed query boundaries;
- Transactional coordination with related marketplace bounded contexts.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Coordinates via transactional events.

## Verification

- `cargo test -p rustok-marketplace-seller`
- `cargo xtask module validate marketplace_seller`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
