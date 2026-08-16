# `rustok-marketplace-commission` Documentation

## Purpose

`rustok-marketplace-commission` owns commission calculation, rate rules, and fee assignment within the RusToK Marketplace family.

## Scope

- Domain models and storage for commission calculation, rate rules, and fee assignment;
- Idempotent command ports and typed query boundaries;
- Transactional coordination with related marketplace bounded contexts.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Coordinates via transactional events.

## Verification

- `cargo test -p rustok-marketplace-commission`
- `cargo xtask module validate marketplace_commission`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
