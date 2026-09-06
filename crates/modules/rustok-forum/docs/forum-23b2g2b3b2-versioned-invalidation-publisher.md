# FORUM-23B2G2B3B2 versioned invalidation publisher

## Status

`source_complete_runtime_evidence_pending`

This slice completed the owner-publisher half of the accepted Forum Search
versioned invalidation rollout. Forum publishes one sealed version-1 typed
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

The remaining runtime-evidence protocol is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json
crates/rustok-forum/docs/forum-23b2g2b3d-runtime-evidence.md
```

## Sealed event family

`rustok-events` owns:

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
envelope ID remains typed-envelope `causation_id`. Search `ingest_sequence`,
locale, channel, visibility snapshots, rendered content, document payload,
reasons, claims and roles are not part of the typed payload.

The family is sealed through `EventContract`, registered through
`event_schema`/`event_schemas`, and represented by one explicit
`ContractEventPayload::ForumSearchProjection` variant. External modules cannot
invent an arbitrary event name or untyped JSON payload.

## PostgreSQL owner transaction

Both active Forum invalidation publisher styles perform the same ordered
sequence:

1. allocate the next tenant-scoped Forum owner revision;
2. publish the mandatory legacy `index.reindex_requested` root envelope and
   retain its exact envelope ID;
3. publish `forum.search_projection.invalidation_issued` with
   `causation_id` equal to that root ID;
4. append `forum_projection_revision_ledger` with the same root ID;
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

The legacy root envelope ID remains the canonical identity in:

```text
forum_projection_revision_ledger.event_id
search_projection_inbox.event_id
ContractEventEnvelope.causation_id
```

The typed envelope has its own transport ID, but that ID is not written to the
Forum owner ledger and is not a second Search projection identity. The Search
consumer adapts the typed delivery through its mandatory `causation_id`, so
legacy and typed delivery converge on the existing root-ID inbox uniqueness
boundary.

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

Adding the family changed three release digests and deliberately left the two
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

## Delivered consumer

`FORUM-23B2G2B3C` was merged through PR #2753. It adds one default-off persistent
Search typed-contract consumer that:

1. receives and acknowledges with the exact persistent cursor;
2. validates the sealed event and mandatory root causation ID;
3. maps that root ID into the existing `search_projection_inbox.event_id`;
4. validates a complete matching durable row before accepting a duplicate;
5. records decode/schema/semantic poison durably before DLQ acknowledgement;
6. reuses the existing Forum reconciler and projector;
7. never creates a second projection lane or compares owner revision with
   `ingest_sequence`.

`FORUM-23B2G2B3D0` now freezes the remaining executable evidence protocol in
`crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json`.
It does not claim that PostgreSQL, Iggy, restart, poison, DLQ or multi-process
evidence has run.

## Compatibility boundary

The delivered publisher and consumer do not:

- change or remove the legacy root event;
- change the root event or root envelope JSON schema;
- change the outbox table or relay;
- add a second Forum publisher path;
- add a second Search inbox or projector;
- advance a Search owner checkpoint on enqueue;
- compare Forum owner revision with Search `ingest_sequence`;
- make Search a synchronous Forum command dependency;
- add an Iggy dependency to `rustok-search`;
- close `FORUM-23` or `LINK-FORUM-03` without runtime evidence.

The persistent consumer remains default-off and requires PostgreSQL plus the
`outbox_iggy` delivery profile when enabled.

## Verification status

Tests, source verifiers, formatting, Cargo checks, Clippy, CI and runtime
evidence were intentionally not executed by the implementation agent.

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-causation-api.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-publisher.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-consumer.mjs
cargo run -p rustok-events --example event_contract_digests
cargo test -p rustok-events
cargo test -p rustok-search forum_contract_ingress -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
```
