# `rustok-marketplace-ledger` Documentation

## Purpose

`rustok-marketplace-ledger` owns seller ledger, escrow accounting, and transaction entries within the RusToK Marketplace family.

## Scope

- Domain models and storage for seller ledger, escrow accounting, and transaction entries;
- Idempotent command ports and typed query boundaries;
- Transactional coordination with related marketplace bounded contexts.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Coordinates via transactional events.

## Verification

- `cargo test -p rustok-marketplace-ledger`
- `cargo xtask module validate marketplace_ledger`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
