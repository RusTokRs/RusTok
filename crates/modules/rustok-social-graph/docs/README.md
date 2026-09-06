# Social Graph Documentation

## Purpose

`rustok-social-graph` is the tenant-scoped owner for social relationships (block, mute, follow) and relation-state events.

## Scope

- Directional relationships: follow, block, mute;
- Idempotent command receipts and relation mutation transactions;
- Authoritative event publishing (`social_graph.relation.state_changed`).

## Integration

- Outbox integration for transactional event persistence;
- Neutral read/write ports consumed by GraphQL, Profiles, and Notifications.

## Verification

- `cargo test -p rustok-social-graph`
- `cargo xtask module validate social_graph`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
