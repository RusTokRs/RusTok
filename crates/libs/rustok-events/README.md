# rustok-events

## Purpose

`rustok-events` owns the canonical event contracts, schemas, and validation rules for RusToK.

## Responsibilities

- Define `DomainEvent`, `EventEnvelope`, and the event schema registry.
- Keep event validation and schema metadata independent from runtime infrastructure.
- Keep a committed release artifact for the registry and all root/typed transport
  wire schemas, so accidental contract drift fails tests.
- Provide a manual, read-only canonical digest admission workflow that archives
  generated/committed artifacts and an exact maintainer-review patch without
  committing or pushing repository changes.
- Provide a stable compatibility path while `rustok-core` keeps transitional re-exports.
- Serve as the single source of truth for event payload evolution policy.
- Define the content-free `translation.target.changed` owner fact used to
  invalidate and repair translation inventory without exposing translated
  values.
- Define sealed `TranslationWorkflowEvent` contracts for content-free
  Translation control-plane lifecycle evidence.
- Define sealed `ReactionsEvent` contracts for committed actor-state changes and
  bounded aggregate repair without exposing producer content or presentation.

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
- `ReactionsEvent`
- `REACTIONS_EVENT_SCHEMAS`
- `TranslationWorkflowEvent`
- `TRANSLATION_WORKFLOW_EVENT_SCHEMAS`

## Interactions

- Used by domain modules that publish or consume typed RusToK events (including
  tenant lifecycle contracts and static-distribution queue, claim, and terminal
  completion evidence plus verified activation, rebuild-only rollback, and
  revocation identity).
- Works with `rustok-core`, which keeps compatibility re-exports during the transition.
- Used by transport-oriented crates such as `rustok-outbox` and `rustok-iggy` through shared event contracts rather than transport-owned schemas.

Root envelopes use the nil tenant UUID only for explicitly platform-capable
`DomainEvent` variants. All other root and typed contract envelopes reject the
sentinel before persistence or relay.

The rebuild-only rollback and `build.rolled_back` contracts describe the
current implementation. The accepted
[module release rollback safety decision](../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
requires an atomic cutover to `rustok-modules`-owned recovery-operation and
desired/observed rollout facts. The cutover removes superseded build rollback
events and every repository-owned caller rather than retaining parallel event
families.

## Docs

- [Module docs](./docs/README.md)
- [Event contract digest admission](./docs/event-contract-digest-admission.md)
- [Event schema release decision](../../DECISIONS/2026-07-23-event-schema-release-discipline.md)
- [Platform docs index](../../docs/index.md)
