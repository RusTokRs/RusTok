# `rustok-marketplace` Documentation

## Purpose

`rustok-marketplace` owns Marketplace family orchestration, root surfaces, and composition within the RusToK Marketplace family.

## Scope

- Domain models and storage for Marketplace family orchestration, root surfaces, and composition;
- Idempotent command ports and typed query boundaries;
- Transactional coordination with related marketplace bounded contexts.

## Integration

- Implements FBA ports consumed by `rustok-marketplace` and `rustok-commerce`;
- Coordinates via transactional events.

## Verification

- `cargo test -p rustok-marketplace`
- `cargo xtask module validate marketplace`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Commerce Plan](../../rustok-commerce/docs/implementation-plan.md)
