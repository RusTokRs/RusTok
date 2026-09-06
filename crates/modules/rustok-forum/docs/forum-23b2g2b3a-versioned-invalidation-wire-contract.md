# FORUM-23B2G2B3A versioned Search invalidation wire contract

## Status

`source_complete_runtime_evidence_pending`

This slice froze the version-1 typed wire contract and rolling-delivery sequence
for Forum Search projection invalidations. The executable event family,
causation-aware owner publisher and persistent Search consumer are now merged;
maintainer-executed PostgreSQL, Iggy and cross-module evidence remains open.

The accepted cross-module decision is:

```text
DECISIONS/2026-07-31-forum-search-versioned-invalidation-rollout.md
```

The machine-readable wire contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json
```

The remaining runtime-evidence protocol is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json
crates/rustok-forum/docs/forum-23b2g2b3d-runtime-evidence.md
```

## Why a new event type

The current release train permits schema version 1 only. The legacy root event
`index.reindex_requested` could not gain an owner revision without changing its
payload contract. The rollout therefore introduced a sealed event type rather
than mutating the established root shape:

```text
family: forum_search_projection
Rust family: ForumSearchProjectionEvent
variant: InvalidationIssued
event type: forum.search_projection.invalidation_issued
schema version: 1
```

The v1 payload contains exactly the causal facts required by the downstream
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

On PostgreSQL, Forum performs one atomic sequence:

1. allocate the next tenant-scoped owner revision;
2. publish the existing `index.reindex_requested` root envelope and retain its
   exact envelope ID;
3. publish `forum.search_projection.invalidation_issued` with
   `causation_id` equal to that root envelope ID;
4. append `forum_projection_revision_ledger` using the same root envelope ID;
5. commit owner state, both outbox rows, the counter and the ledger row together.

Failure at any point rolls back the complete owner transaction. The typed event
is not emitted separately after commit, and the ledger does not switch to the
typed envelope ID.

SQLite remains validation-only for this projection boundary. It has no matching
owner revision ledger or PostgreSQL Search background reconciler and therefore
does not dual-publish the typed invalidation.

## Why legacy publication remains

The legacy root envelope ID is the canonical identity in:

```text
forum_projection_revision_ledger.event_id
search_projection_inbox.event_id
ContractEventEnvelope.causation_id
```

This release train keeps legacy publication mandatory. The typed event adds an
explicit remote-consumer contract but does not replace the root identity. Any
future retirement requires a separate accepted ADR and runtime evidence proving
a new stable projection identity, replay behavior and checkpoint migration.

## One Search inbox and one projector

The typed consumer does not create a parallel Search execution lane. It adapts
the validated typed event into the existing root-compatible Forum inbox
representation:

```text
legacy ingress inbox identity = EventEnvelope.id
typed ingress inbox identity  = ContractEventEnvelope.causation_id
shared identity               = legacy root envelope ID
```

`search_projection_inbox` keeps its existing unique `event_id` boundary.
Whichever representation arrives first creates the one durable work item; the
other must match the complete retained identity and becomes a durable duplicate.
Both converge on the existing `ForumProjectionInbox`,
`ForumProjectionReconciler` and `ForumSearchProjector` path.

The typed envelope's own ID is retained only for the persistent transport and
poison/DLQ receipts. It is never used as a second projection identity or a
second owner revision.

## Persistent cursor and poison policy

The Search consumer uses the existing persistent typed-contract cursor contract.
Receive and acknowledgement use the same cursor.

A valid Forum Search invalidation can be acknowledged only after:

- the shared inbox row is durably inserted; or
- the exact existing durable row for the shared root identity is recognized.

A decode, schema or semantic poison delivery can be acknowledged only after:

- a durable connector-owned poison receipt is stored for the exact broker
  message identity; and
- DLQ publication succeeds or a previous successful deterministic publication
  is durably recognized.

Transient database, transport or acknowledgement failure leaves the exact
source offset uncommitted. The owner process fails or retries under its
supervisor rather than falling back to an in-memory consumer.

Unrelated sealed event families may be ignored only through the validated
persistent-cursor path; they are not decoded as Forum events.

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
revision checkpoint advances only after projection state commits or a
missing/dead-lettered delivery is repaired successfully.

## Delivered rollout slices

### `FORUM-23B2G2B3B1`

PR #2741 added causation-aware `ContractEventEnvelope` construction and
transactional typed-contract publication APIs without changing the established
root event schema.

### `FORUM-23B2G2B3B2`

PR #2749 added the sealed event family, registry/digest release artifact and
atomic PostgreSQL Forum dual publisher while retaining legacy root and ledger
identity.

### `FORUM-23B2G2B3C`

PR #2753 added the default-off persistent Search typed-contract consumer, exact
one-inbox identity validation and connector-owned durable poison/DLQ handling.

### `FORUM-23B2G2B3D`

Runtime evidence remains open. `FORUM-23B2G2B3D0` freezes the required
executable scenarios and artifact fields without claiming that they have run.

## Compatibility boundary

The merged rollout does not:

- remove or weaken the legacy root event;
- compare Forum owner revision with Search `ingest_sequence`;
- add a second Search inbox, reconciler, projector or watermark;
- add an Iggy dependency to `rustok-search`;
- make Search a synchronous Forum command dependency;
- close `FORUM-23` or `LINK-FORUM-03` without runtime evidence.

The persistent consumer remains default-off and requires PostgreSQL plus the
`outbox_iggy` delivery profile when enabled.

## Maintainer verification

The implementation agent did not run these commands:

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-wire.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-causation-api.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-publisher.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-consumer.mjs
cargo run -p rustok-events --example event_contract_digests
cargo test -p rustok-events
cargo test -p rustok-search forum_contract_ingress -- --nocapture
cargo check -p rustok-forum -p rustok-search -p rustok-outbox
cargo check -p rustok-server --features mod-forum --all-targets
cargo clippy -p rustok-events -p rustok-forum -p rustok-search -p rustok-outbox --all-targets -- -D warnings
```
