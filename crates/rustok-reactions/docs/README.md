# Reactions module contract

## Purpose

`rustok-reactions` is the first-party owner for configurable, tenant-scoped reactions across entity subjects.

## Scope

- Reaction subjects, catalogs, actor states, and aggregate counts;
- Idempotent command receipts and atomic transition facts;
- Reconciliation inspection and repair.

## Integration

- Depends on Outbox for shared durable owner-operation receipts and `rustok-events` for semantic events;
- Exposes neutral ports consumed by domain modules.

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)

`rustok-reactions` is an optional first-party owner. It depends on Outbox for
the shared durable owner-operation receipt ledger and on `rustok-events` for the
sealed semantic event family.

## Runtime composition

Module registration initializes immediate and deferred
`ReactionSubjectProvider` registries. A producer provider authorizes the exact
tenant/source/kind/subject/revision and returns the bounded reaction catalog.
Missing providers and unavailable subjects fail closed.

## Persistence boundary

The owner schema contains:

- `reaction_subjects`: one tenant-composite serialization row per producer
  subject;
- `reaction_catalogs`: immutable JSON catalog snapshots by subject and revision;
- `reaction_actor_states`: one bounded selected-key set per subject and actor;
- `reaction_aggregates`: one non-negative count per subject and reaction key.

All child tables carry `tenant_id` and use tenant-composite foreign keys. The
schema deliberately does not store producer routes, titles, visibility, content
or profile presentation.

## Command boundary

`ReactionWritePort::apply_reaction` requires deadline and idempotency semantics.
The command UUID must equal `PortContext.idempotency_key`. User callers may only
act as their own UUID; service/system callers require `reactions:act_as_actor`.

The service authorizes the producer subject before persistence, admits the
shared Outbox receipt, synchronizes the immutable catalog snapshot, serializes
the subject row, updates actor state and aggregate deltas, writes one sealed
`reactions.actor_state.changed` fact for a real transition and completes the
receipt in one transaction. The exact envelope UUID is the admitted owner
operation UUID. Event conflict/unavailability aborts the owner transaction. A
rolled-back command persists a terminal typed receipt failure only after that
transaction is released.

Single-selection replaces the previous key atomically. Multiple-selection is
bounded by the authorized catalog. Removing an absent key and adding an already
selected key are idempotent no-ops and do not emit events. Completed receipt
replay also emits no new fact.

## Reconciliation boundary

`inspect_reconciliation` and `repair_reconciliation` require exact tenant scope
and `reactions:reconcile`. Repair additionally requires a non-nil command UUID
equal to the write idempotency key and uses the shared receipt ledger.

Inspection is read-only and bounded to one subject, at most 1,000 actor states,
128 aggregate rows and 64 reported issues. Repair serializes the subject and
reconstructs only `reaction_aggregates` from valid actor selections under the
immutable current catalog. A drift repair writes one sealed
`reactions.subject.reconciled` fact and completes its receipt atomically. A clean
repair is a receipt-only no-op.

Missing/corrupt current catalog, non-positive actor-state revision, corrupt or
duplicate selections, selection-limit violations and keys outside the catalog
block mutation. Repair never changes actor selections, catalog snapshots or
producer-private state.

## Catalog compatibility

The initial catalog revision equals the producer-authorized subject revision.
A revision cannot be rebound to different catalog JSON. Catalog advancement
cannot remove a key with live aggregate state; reconciliation must happen first.

## Exclusions

No background repair worker/scheduler, second producer adapter, transport, UI,
default enablement, reputation, achievement or Forum vote migration is included.
The committed event-contract digest artifact and `Cargo.lock` remain maintainer-
generated.

## Verification

```bash
cargo test -p rustok-events reactions
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo check -p rustok-reactions --all-targets
cargo run -p rustok-events --example event_contract_digests -- --write
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-reactions-events-reconciliation.mjs
git diff --check
```

Tests, checks, digest/lockfile generation and runtime evidence are maintainer-run.
