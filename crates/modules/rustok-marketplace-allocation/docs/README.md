# `rustok-marketplace-allocation` Documentation

## Purpose

`rustok-marketplace-allocation` owns multi-seller order allocation and line assignment within the RusToK Marketplace family.

## Scope

- Domain models and storage for multi-seller order allocation and line assignment;
- Idempotent command ports and typed query boundaries;
- Transactional coordination with related marketplace bounded contexts.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Coordinates via transactional events.

## Verification

- `cargo test -p rustok-marketplace-allocation`
- `cargo xtask module validate marketplace_allocation`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
