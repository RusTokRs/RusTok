# FORUM-23B2G2B3B2 versioned invalidation publisher

## Status

`source_complete_consumer_pending`

This slice completes the owner-publisher half of the accepted Forum Search
versioned invalidation rollout. Forum now publishes one sealed version-1 typed
contract beside the existing root invalidation inside the same PostgreSQL owner
transaction.

The accepted rollout remains defined by:

```text
DECISIONS/2026-07-31-forum-search-versioned-invalidation-rollout.md
crates/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json
```

The machine-readable implementation result is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-publisher.json
```

## Sealed event family

`rustok-events` now owns:

```text
Rust family: ForumSearchProjectionEvent
transport family: forum_search_projection
variant: InvalidationIssued
event type: forum.search_projection.invalidation_issued
schema version: 1
```

The payload contains only:

```text
owner_revision: positive int64
target_type: forum | forum_category | forum_topic
target_id: null for forum; non-nil UUID for forum_category/forum_topic
```

Tenant and optional actor remain envelope metadata. The exact predecessor root
envelope id remains typed-envelope `causation_id`. Search `ingest_sequence`,
locale, channel, visibility snapshots, rendered content, document payload,
reasons, claims and roles are not part of the typed payload.

The family is sealed through `EventContract`, registered through
`event_schema`/`event_schemas`, and represented by one explicit
`ContractEventPayload::ForumSearchProjection` variant. External modules cannot
invent an arbitrary event name or untyped JSON payload.

## PostgreSQL owner transaction

Both active Forum invalidation publisher styles now perform the same ordered
sequence:

1. allocate the next tenant-scoped Forum owner revision;
2. publish the mandatory legacy `index.reindex_requested` root envelope and
   retain its exact envelope id;
3. publish `forum.search_projection.invalidation_issued` with
   `causation_id` equal to that root id;
4. append `forum_projection_revision_ledger` with the same root id;
5. commit owner state, revision counter, both outbox rows and ledger row
   together.

The direct transaction-only helper uses:

```text
TransactionalEventBus::publish_root_in_tx_with_envelope_id
TransactionalEventBus::publish_contract_direct_in_tx_with_causation_and_envelope_id
```

The composed bus path uses:

```text
TransactionalEventBus::publish_in_tx_with_envelope_id
TransactionalEventBus::publish_contract_in_tx_with_causation
```

Both paths use the canonical `sys_events` outbox writer. No direct broker,
process-local fallback, second outbox table or second owner clock is introduced.

A typed publication failure occurs before the owner-ledger append and fails the
same database transaction. It therefore cannot leave a committed owner revision,
legacy root or owner mutation without the required typed counterpart.

## Identity and ordering

The legacy root envelope id remains the canonical identity in:

```text
forum_projection_revision_ledger.event_id
search_projection_inbox.event_id
```

The typed envelope has its own transport id, but that id is not written to the
Forum owner ledger and is not a second Search projection identity. The future
Search consumer must adapt the typed delivery through its mandatory
`causation_id`, so legacy and typed delivery converge on the existing root-id
inbox uniqueness boundary.

Forum owner revision and Search-owned `ingest_sequence` remain independent:

```text
Forum revision: causal owner order and gap detection
Search ingest_sequence: durable delivery claim order
```

The values are never compared numerically.

## Non-PostgreSQL behavior

The owner revision ledger and background Search reconciler are PostgreSQL-only.
This slice therefore does not emit the typed contract on SQLite or other
non-PostgreSQL profiles.

The composed non-PostgreSQL path retains its historical legacy root publication.
The transaction-only validation path retains root payload validation without
inventing an owner revision, ledger row or typed event that has no matching
runtime consumer.

## Digest release gate

Adding the family changes three release digests and deliberately leaves the two
root-wire digests unchanged:

```text
registry:          sha256:a4b41305240a06ad57bb10499f6699226e5fe77adff7d6efbafe83c9e84ae0aa
root event:        sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87
root envelope:     sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d
contract payload:  sha256:e07934a82cb82ae14ec3d8b7c1e5938a6dd4bafdd563ebdce7e890985bd8011d
contract envelope: sha256:e0466ed18a986885f62f08f866a882b1f2de9ed277c6e8a29d04f43aaa705d5d
```

The implementation environment did not have the repository Rust toolchain and
did not execute the Cargo example. The exact repository algorithm was reproduced
from `schema.rs`, `schemars 1.2.1` derive/generator behavior, sorted registry
serialization and SHA-256 canonical JSON. Before computing the new values, that
model reproduced all five committed pre-change digests exactly. The committed
artifact therefore contains deliberate release values rather than placeholders
or guessed hashes.

Maintainers may independently regenerate the artifact with:

```bash
cargo run -p rustok-events --example event_contract_digests -- --write
```

## Compatibility boundary

This slice does not:

- change or remove the legacy root event;
- change the root event or root envelope JSON schema;
- change the outbox table or relay;
- add a second Forum publisher path;
- add a Search inbox or projector;
- advance a Search owner checkpoint on enqueue;
- add dependencies or change `Cargo.lock`;
- claim remote transport or PostgreSQL runtime evidence.

`FORUM-23` and `LINK-FORUM-03` remain open.

## Next slice

`FORUM-23B2G2B3C` must add one persistent Search typed-contract consumer that:

1. receives and acknowledges with the exact persistent cursor;
2. validates the sealed event and mandatory root causation id;
3. maps that root id into the existing `search_projection_inbox.event_id`;
4. treats an existing row as a durable duplicate;
5. records decode/schema/semantic poison durably before DLQ acknowledgement;
6. reuses the existing Forum reconciler and projector;
7. never creates a second projection lane or compares owner revision with
   `ingest_sequence`.

## Verification status

Tests, source verifiers, formatting, Cargo checks, Clippy, CI and runtime
evidence were intentionally not executed by the implementation agent.
