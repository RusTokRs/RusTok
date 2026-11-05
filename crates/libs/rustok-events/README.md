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
- `InventoryReservationEvent` / `INVENTORY_RESERVATION_EVENT_SCHEMAS`
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

Localized root events and Forms submission events use the platform locale
length bound from `rustok_api::locale::MAX_LOCALE_TAG_LEN`. Publishing owners retain
locale grammar and canonicalization policy. The
[owner plan](docs/implementation-plan.md#locale-length-validation-2026-11-05)
records the regression and its verification scope.

The rebuild-only rollback and `build.rolled_back` contracts describe the
current implementation. The accepted
[module release rollback safety decision](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
requires an atomic cutover to `rustok-modules`-owned recovery-operation and
desired/observed rollout facts. The cutover removes superseded build rollback
events and every repository-owned caller rather than retaining parallel event
families.

## Docs

- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md)
- [Module docs](./docs/README.md)
- [Event contract digest admission](./docs/event-contract-digest-admission.md)
- [Event schema release decision](../../../DECISIONS/2026-07-23-event-schema-release-discipline.md)
- [Platform docs index](../../../docs/index.md)

## Known Limitations / Pending Implementation

The new quantity-free sealed `inventory.reservation.changed` v1 family is registered in source;
only non-nil Product/Variant UUIDs enter its payload. It neither changes root `InventoryUpdated`
nor borrows Product's refresh revision. The committed release digests remain unchanged: canonical
Rust regeneration is blocked by missing Cargo/rustc and must include the existing pending
Checkout-family changes. Four new family regression definitions are uncompiled/unrun. See the
[digest admission gates](docs/event-contract-digest-admission.md#pending-inventory-reservation-admission-2026-11-01)
and [Inventory source packet](../../modules/rustok-inventory/contracts/evidence/reservation-events-source.json).
Transport/runtime certification belongs to Core/Outbox/server and consumers, not this registry.

## Inventory source continuation (2026-11-02; uncompiled)

The additive sealed source family uses retained change/baseline UUIDs and Inventory revisions,
not Product revisions/copied stock. Compiled required-module policy refuses unmatched Inventory
source/reservation facts before broker-only success; Product remote-only routes remain supported.
No production source/Index listener is installed by schema registration. See
[API](CRATE_API.md#inventory-source-facts-and-delivery-policy-2026-11-02-source-only) and
[pending admission](docs/event-contract-digest-admission.md#pending-inventory-source-admission-2026-11-02).
Six family definitions and all downstream definitions remain uncompiled/unrun.
