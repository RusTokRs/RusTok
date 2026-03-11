# rustok-events documentation

## Purpose

`rustok-events` provides a stable, shared import surface for RusToK domain event contracts.
At the current stage it acts as an extraction layer that re-exports event primitives from
`rustok-core` while preserving backward compatibility for downstream crates.

## Responsibilities

- Maintain a stable API path for event contracts (`DomainEvent`, `EventEnvelope`).
- Decouple consumer crates from direct event-type imports in `rustok-core`.
- Support incremental migration of event contracts from core to dedicated ownership.

## Interactions

- **Upstream dependency:** `rustok-core` (current source of event type definitions).
- **Downstream consumers:** domain modules, transport/runtime crates, and test utilities
  that exchange events through RusToK event flows.
- **Architecture alignment:** event contracts are aligned with centralized event flow
  documentation and outbox/runtime guidance.

## Entry points

- `rustok_events::DomainEvent`
- `rustok_events::EventEnvelope`
- Compatibility exports:
  - `rustok_events::RootDomainEvent`
  - `rustok_events::RootEventEnvelope`

## Related docs

- [Implementation plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
