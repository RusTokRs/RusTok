# ADR: Forum Search versioned invalidation rollout

- Status: accepted
- Date: 2026-07-31

## Context

Forum already publishes the legacy root event `index.reindex_requested` inside
the owner transaction. Search persists that envelope in one durable inbox,
orders execution with a Search-issued `ingest_sequence`, and reconciles the
result against a Forum-issued monotonic owner revision ledger.

The owner revision is not present in the legacy root payload. A versioned typed
wire fact is required for remote consumers, but adding it must not create a
second Search projection executor, a second owner clock, or an event identity
that cannot be reconciled with the existing inbox and checkpoint.

The current event release train accepts schema version 1 only. A new payload is
therefore a new sealed event type rather than a mutation of
`index.reindex_requested`.

## Decision

`rustok-events` will own a sealed `ForumSearchProjectionEvent` family with one
version-1 variant:

```text
forum.search_projection.invalidation_issued
```

The payload contains only:

- positive `owner_revision`;
- `target_type` in `forum|forum_category|forum_topic`;
- nullable `target_id`, null only for `forum` and required otherwise.

Tenant and actor remain envelope metadata. The typed envelope's
`causation_id` is mandatory and equals the exact legacy
`index.reindex_requested` root envelope ID recorded in
`forum_projection_revision_ledger.event_id`.

During this release train Forum publishes both representations in one owner
transaction in this order:

1. allocate the next Forum owner revision;
2. publish the legacy root envelope and retain its ID;
3. publish the typed contract envelope caused by that root ID;
4. append the owner ledger row using the root ID;
5. commit all owner state and both outbox rows atomically.

Any failure rolls back the complete transaction. PostgreSQL is the only runtime
profile for this dual publication because the owner revision ledger and Search
reconciler are PostgreSQL-only.

The legacy root publication remains mandatory until a future accepted ADR names
its retirement evidence. The typed event is an additional transport contract,
not a replacement identity in this milestone.

Search will use one projection inbox and one projector. The typed consumer maps
`ContractEventEnvelope.causation_id` to the existing
`search_projection_inbox.event_id`; the legacy consumer uses the root envelope
ID, which is the same value. The existing uniqueness boundary therefore
collapses both deliveries into one durable work item. The typed envelope's own
ID is retained only for transport receipt, DLQ, and diagnostics.

A valid typed delivery is acknowledged only after the shared inbox insert or a
durable duplicate result. Decode/schema/semantic poison is acknowledged only
after a durable DLQ receipt and one successful DLQ publication. Transient
storage, projection, or acknowledgement failure leaves the exact persistent
cursor offset uncommitted. No process-local fallback or second projector is
permitted.

## Consequences

- owner revision and Search `ingest_sequence` remain independent counters;
- the typed payload can carry causal owner order without changing the legacy
  root schema;
- dual delivery is at-least-once but has one Search execution identity;
- the Forum ledger remains the complete revision-to-root-event history;
- Search checkpoint advancement remains checkpoint-after-projection-success;
- event registry, digest artifact, publisher, persistent consumer, DLQ receipt,
  and runtime evidence are separate implementation slices following this
  contract freeze;
- `FORUM-23` and `LINK-FORUM-03` remain open until implementation and maintainer
  runtime evidence are complete.
