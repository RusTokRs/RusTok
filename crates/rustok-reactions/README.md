# rustok-reactions

## Purpose

`rustok-reactions` is the optional shared owner for reusable reactions across
revisioned subjects owned by other RusToK modules.

## Current source slice

The module now provides:

- source-owned subject authorization through `rustok-reactions-api`;
- PostgreSQL/SQLite-compatible SeaORM migrations for tenant-composite owner state;
- immutable catalog snapshots bound to an explicit catalog revision;
- one bounded actor state per tenant, subject, and actor;
- aggregate counts updated in the same owner transaction as actor state;
- durable idempotency through the shared `rustok-outbox` owner-operation receipt
  ledger;
- sealed `rustok-events` semantic facts for changed actor state and committed
  aggregate repair;
- bounded owner-only inspection and aggregate reconciliation;
- transport-neutral `ReactionReadPort` and `ReactionWritePort` implementations.

A reaction command UUID must equal the port idempotency key. The owner admits
that key through Outbox, authorizes the subject through its producer provider,
serializes the subject row, applies actor state and aggregate changes, writes one
semantic event for a real change, completes the receipt in the same transaction,
and persists a typed terminal failure after rollback when needed. Idempotent
no-op commands and completed receipt replays do not publish another event.

Reconciliation requires exact tenant scope plus `reactions:reconcile`. Inspection
is read-only. Repair is idempotent, bounded to one stored subject and rebuilds
only aggregate rows from valid actor states under the immutable current catalog.
Catalog or actor-state corruption blocks repair rather than silently changing
user selections.

## Ownership

Reactions owns catalog snapshots, actor reaction state, aggregate projections,
semantic reaction events and aggregate repair. Producer modules own subject
existence, current revision, visibility, lifecycle and reaction policy. Profiles
owns actor presentation. Reputation, achievements and Notifications are
separate capabilities.

The Reactions owner never reads producer-private tables (Forum, Blog,
Comments, Profiles, Media, Groups or Commerce private tables).

## Deliberate limits

This slice does not add a background reconciliation worker, scheduler, second
producer adapter, GraphQL, REST, native server functions, UI packages, default
enablement, Forum vote migration, reputation or achievements.

The initial catalog revision is the producer-authorized subject revision. A
future independent catalog-revision contract requires an explicit API change and
migration rather than implicit reinterpretation.

## Degraded mode

The module remains optional and outside `default_enabled`. When it is absent,
producer owner commands remain available and reaction UI stays hidden.
Existing Forum votes remain unchanged.

## Entry points

- `ReactionsModule`
- `ReactionsService`
- `rustok_reactions_api::{ReactionReadPort, ReactionWritePort}`
- `ReactionReconciliationRequest`
- `RepairReactionSubjectCommand`
- `migrations::migrations()`

See [module contract](docs/README.md) and
[implementation plan](docs/implementation-plan.md).
