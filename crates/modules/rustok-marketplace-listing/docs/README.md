# `rustok-marketplace-listing` Documentation

## Purpose

`rustok-marketplace-listing` owns seller listing catalog, approval, and channel visibility within the RusToK Marketplace family.

## Scope

- Domain models and storage for seller listing catalog, approval, and channel visibility;
- Idempotent command ports and typed query boundaries;
- Transactional coordination with related marketplace bounded contexts.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Coordinates via transactional events.

## Verification

- `cargo test -p rustok-marketplace-listing`
- `cargo xtask module validate marketplace_listing`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
