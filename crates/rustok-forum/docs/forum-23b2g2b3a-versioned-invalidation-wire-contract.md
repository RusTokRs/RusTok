# FORUM-23B2G2B3A versioned Search invalidation wire contract

## Status

`contract_frozen_implementation_pending`

This slice freezes the version-1 typed wire contract and rollout sequence for
Forum Search projection invalidations. It deliberately does not modify the
sealed event registry, digest artifact, Forum publisher, Search inbox, Search
worker, Iggy consumer, or projection execution.

The accepted cross-module decision is:

```text
DECISIONS/2026-07-31-forum-search-versioned-invalidation-rollout.md
```

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json
```

## Why a new event type

The current release train permits schema version 1 only. The legacy root event
`index.reindex_requested` cannot gain an owner revision without changing its
payload contract. The rollout therefore introduces a new sealed event type
rather than mutating the established root shape:

```text
family: forum_search_projection
Rust family: ForumSearchProjectionEvent
variant: InvalidationIssued
event type: forum.search_projection.invalidation_issued
schema version: 1
```

The v1 payload contains exactly the causal facts required by a downstream
projection owner:

```text
owner_revision: positive int64
target_type: forum | forum_category | forum_topic
target_id: null for forum; non-nil UUID for forum_category/forum_topic
```

Tenant and actor remain typed-envelope metadata. Locale, channel, visibility,
rendered content, document payload, reason, claims, roles and Search
`ingest_sequence` do not belong in the event.

## One owner transaction

After implementation is enabled on PostgreSQL, Forum must perform one atomic
sequence:

1. allocate the next tenant-scoped owner revision;
2. publish the existing `index.reindex_requested` root envelope and retain its
   exact envelope ID;
3. publish `forum.search_projection.invalidation_issued` with
   `causation_id` equal to that root envelope ID;
4. append `forum_projection_revision_ledger` using the same root envelope ID;
5. commit owner state, both outbox rows, the counter and the ledger row together.

Failure at any point rolls back the complete owner transaction. The typed event
must not be emitted separately after commit, and the ledger must not switch to
the typed envelope ID.

SQLite remains validation-only for this projection boundary. It has no matching
owner revision ledger or PostgreSQL Search background reconciler and therefore
does not dual-publish the typed invalidation.

## Why legacy publication remains

The legacy root envelope ID is already the canonical identity in:

```text
forum_projection_revision_ledger.event_id
search_projection_inbox.event_id
```

This release train keeps legacy publication mandatory. The typed event adds an
explicit remote-consumer contract but does not replace the root identity. Any
future retirement requires a separate accepted ADR and evidence proving a new
stable projection identity, replay behavior and checkpoint migration.

## One Search inbox and one projector

The future typed consumer must not create a parallel Search execution lane.
Instead it adapts the validated typed event into the existing root-compatible
Forum inbox representation:

```text
legacy ingress inbox identity = EventEnvelope.id
typed ingress inbox identity  = ContractEventEnvelope.causation_id
shared identity               = legacy root envelope ID
```

`search_projection_inbox` already uses `ON CONFLICT (event_id) DO NOTHING`.
Whichever representation arrives first creates the one durable work item; the
other is a duplicate of the same owner invalidation. Both therefore converge on
the existing `ForumProjectionInbox`, `ForumProjectionReconciler` and
`ForumSearchProjector` path.

The typed envelope's own ID is retained only for the persistent transport
receipt, poison/DLQ receipt and diagnostics. It is never used as a second
projection identity or a second owner revision.

## Persistent cursor and poison policy

The future Search consumer must use the existing persistent typed-contract
consumer cursor contract. Receive and acknowledgement use the same cursor.

A valid Forum Search invalidation can be acknowledged only after:

- the shared inbox row is durably inserted; or
- the existing durable row for the shared event identity is recognized.

A decode, schema or semantic poison delivery can be acknowledged only after:

- a durable poison/DLQ receipt is stored for the exact broker message identity;
- the DLQ payload is published once or a previous successful publication is
  durably recognized.

Transient database, transport, projection or acknowledgement failure leaves the
exact source offset uncommitted. The owner process must fail or retry under its
supervisor rather than silently falling back to an in-memory consumer.

Unrelated sealed event families may be ignored by this dedicated consumer only
through the normal validated persistent-cursor path; they must not be decoded as
Forum events.

## Independent clocks

The typed payload carries Forum causal order:

```text
forum_projection_revision_ledger.revision
```

Search delivery execution continues to use:

```text
search_projection_inbox.ingest_sequence
```

These values remain independent and are never compared numerically. The owner
revision checkpoint still advances only after the projection state commits or a
missing/dead-lettered delivery is repaired successfully.

## Planned implementation slices

### `FORUM-23B2G2B3B`

Add `ForumSearchProjectionEvent` to `rustok-events`, register its v1 schema,
update the committed registry and wire-schema digest artifact, add an additive
causation-aware typed-envelope publication API, and dual-publish from the Forum
owner transaction while retaining legacy root and ledger identity.

### `FORUM-23B2G2B3C`

Add the Search-owned persistent typed-contract consumer, durable poison/DLQ
receipt and one-inbox adapter keyed by the legacy root causation ID. It must
reuse the existing inbox claimant and projector.

### `FORUM-23B2G2B3D`

Capture maintainer-executed PostgreSQL and persistent-cursor evidence for normal
delivery, root-first and typed-first duplicate races, restart, poison, DLQ
publication, acknowledgement failure, missing delivery repair, multi-process
serialization, deletion/ACL ordering and `LINK-FORUM-03`.

## Current-slice boundary

G2B3A does not:

- add `ForumSearchProjectionEvent` or a `ContractEventPayload` variant;
- change `event-contract-digests.json`;
- change `projection_invalidation.rs`;
- publish a typed outbox row;
- add a Search persistent contract consumer;
- change the Search inbox schema or projection executor;
- close `FORUM-23` or `LINK-FORUM-03`.

The canonical tasks remain `in_progress` because implementation and runtime
evidence are still pending.

## Maintainer verification

The implementation agent did not run these commands:

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-wire.mjs
cargo run -p rustok-events --example event_contract_digests
cargo test -p rustok-events
cargo check -p rustok-forum -p rustok-search -p rustok-outbox
cargo clippy -p rustok-events -p rustok-forum -p rustok-search -p rustok-outbox --all-targets -- -D warnings
```

The event digest generator is intentionally deferred to the executable registry
slice; G2B3A changes no Rust event schema and therefore must leave the committed
digest artifact unchanged.
