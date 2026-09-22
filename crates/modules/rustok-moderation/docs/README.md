# Moderation Module Documentation

## Purpose

`rustok-moderation` is the platform moderation owner for reports, cases, reviews, decisions, and appeals.

## Scope

- Content reporting, automated screening, and case queue management;
- Moderator action logs, audit trails, and appeals;
- Domain-agnostic subject references and enforcement hooks.

## Integration

- Connects to domain modules (Forum, Blog, Comments, Profiles) via neutral moderation ports;
- Emits moderation events for domain-local enforcement.

## Verification

- `cargo test -p rustok-moderation`
- `cargo xtask module validate moderation`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
