# Documentation `rustok-events`

`rustok-events` is the canonical shared import surface for platform event
contracts. It owns `DomainEvent`, `EventEnvelope`, schema metadata and
validation rules, while `rustok-core` retains only a compatibility re-export path.

## Purpose

- publish a unified event-contract layer for the platform;
- keep schema metadata, envelope shape and validation rules inside a separate module;
- decouple event consumers from a direct dependency on `rustok-core::events`.

## Responsibilities

- `DomainEvent`, `EventEnvelope`, `EventSchema`, `FieldSchema` and schema registry;
- validation rules and versioning policy for event payloads;
- compatibility aliases and non-breaking migration path for consumers;
- contract tests and release-gate expectations for event-schema changes;
- absence of transport-specific event delivery logic.

## Integration

- `rustok-core::events` remains a compatibility adapter over the canonical surface from `rustok-events`;
- domain modules, outbox/runtime crates and test utilities must import event contracts directly from `rustok-events`;
- changes to event contracts must be synchronized with outbox, replay, DLQ and reindex guidance;
- tenant lifecycle contracts (`tenant.created`, `tenant.updated`, `tenant.module.toggled`) must remain synchronized with tenancy modules and their outbox mutation paths;
- breaking payload changes require a version bump and an explicit dual-read/migration plan.

## Verification

- `cargo xtask module validate events`
- `cargo xtask module test events`
- targeted tests for schema coverage, validation, versioning and envelope JSON roundtrip

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
