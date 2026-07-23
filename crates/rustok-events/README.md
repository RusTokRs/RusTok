# rustok-events

## Purpose

`rustok-events` owns the canonical event contracts, schemas, and validation rules for RusToK.

## Responsibilities

- Define `DomainEvent`, `EventEnvelope`, and the event schema registry.
- Keep event validation and schema metadata independent from runtime infrastructure.
- Keep a committed release artifact for the registry and all root/typed transport
  wire schemas, so accidental contract drift fails tests.
- Provide a stable compatibility path while `rustok-core` keeps transitional re-exports.
- Serve as the single source of truth for event payload evolution policy.

## Entry points

- `DomainEvent`
- `EventEnvelope`
- `EventSchema`
- `FieldSchema`
- `EventContractDigests`
- `event_contract_digests`
- `event_schema`
- `EVENT_SCHEMAS`
- `ValidateEvent`
- `EventValidationError`

## Interactions

- Used by domain modules that publish or consume typed RusToK events (including
  tenant lifecycle contracts and static-distribution queue, claim, and terminal
  completion evidence plus verified activation, rebuild-only rollback, and
  revocation identity).
- Works with `rustok-core`, which keeps compatibility re-exports during the transition.
- Used by transport-oriented crates such as `rustok-outbox` and `rustok-iggy` through shared event contracts rather than transport-owned schemas.

## Docs

- [Module docs](./docs/README.md)
- [Event schema release decision](../../DECISIONS/2026-07-23-event-schema-release-discipline.md)
- [Platform docs index](../../docs/index.md)
