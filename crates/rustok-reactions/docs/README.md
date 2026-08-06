# Reactions module contract

`rustok-reactions` is an optional first-party owner. It depends on Outbox for
the shared durable owner-operation receipt ledger.

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
the subject row, updates actor state and aggregate deltas in one transaction, and
completes the receipt in that transaction. A rolled-back command persists a
terminal typed receipt failure outside the owner transaction.

Single-selection replaces the previous key atomically. Multiple-selection is
bounded by the authorized catalog. Removing an absent key and adding an already
selected key are idempotent no-ops.

## Catalog compatibility

The initial catalog revision equals the producer-authorized subject revision.
A revision cannot be rebound to different catalog JSON. Catalog advancement
cannot remove a key with live aggregate state; reconciliation must happen first.

## Exclusions

No producer adapter, event catalog, outbox event write, reconciliation worker,
transport, UI, default enablement, reputation, achievement or Forum vote
migration is included.

## Verification

```bash
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo check -p rustok-reactions --all-targets
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
git diff --check
```

Tests, checks and runtime evidence are maintainer-run. `Cargo.lock` regeneration
is also maintainer-run for this source slice.
