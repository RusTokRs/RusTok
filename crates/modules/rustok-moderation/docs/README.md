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
- [Application operation migration contract](./application-operation-migration-contract.md)
- [Forum lost response PostgreSQL contract](./forum-lost-response-postgres-contract.md)
- [Host composition executable contract](./host-composition-executable-contract.md)
- [PostgreSQL application operation contract](./postgres-application-operation-contract.md)
- [PostgreSQL dispatch contract](./postgres-dispatch-contract.md)
- [PostgreSQL owner contract](./postgres-owner-contract.md)
- [PostgreSQL recovery contract](./postgres-recovery-contract.md)
- [PostgreSQL scheduler contract](./postgres-scheduler-contract.md)
