# FORUM-23B2G2A Search owner-revision source

## Status

`source_complete_consumer_reconciliation_pending`

This slice exposes the existing immutable Forum event journal as a bounded,
transport-neutral owner-revision source for Search. It does not add another
revision counter and it does not change the already delivered Search inbox
`ingest_sequence` ordering.

## Owner clock

The authoritative revision is:

```text
forum_domain_events.sequence_no
```

`FORUM-09` already owns this append-only BIGINT sequence. The sequence is global
while reads are tenant-scoped, so gaps between successive rows for one tenant
are expected and valid.

No migration, sequence, timestamp ordering rule, UUID ordering rule or new
`DomainEvent` field is introduced.

## Forum owner boundary

`ForumEventService::list_projection_owner_revisions` reads at most 100 journal
rows after an exclusive `after_owner_revision` cursor and orders them by
`sequence_no ASC`.

The owner-facing result contains only:

- owner revision;
- event identity;
- bounded event type;
- projection impact.

Journal payload, actor identity and the RBAC-facing event response are not
exported through the Search capability.

The owner explicitly marks vote, subscription and mention records as
`NoProjectionChange`. Every other current or future admitted Forum journal event
fails safe to `FullRebuild` until the owner classifies it more narrowly.

## Neutral Search contract

Search owns `ForumProjectionOwnerRevisionSourcePort` and validates every page
before it can be consumed:

- tenant UUID must be non-nil;
- cursor must be non-negative;
- page size must be between 1 and 100;
- returned rows must not exceed the requested page size;
- owner revisions must be strictly increasing after the cursor;
- event IDs must be non-nil;
- event types must remain inside the published 96-character journal bound.

Missing host composition and malformed owner pages fail closed through typed
`PortError` values.

The host adapter is the only composition point between Search and Forum. Search
does not query `forum_domain_events` directly.

## Deliberate boundary with G1

Search `ingest_sequence` remains a delivery-order cursor owned by the durable
Search inbox. Forum `sequence_no` is source-of-truth owner order. They are
separate monotonic domains and must not be compared numerically.

This source slice registers the owner port but does not yet connect it to the
background sweeper. In particular, it does not:

- create or advance an owner-revision watermark;
- infer that a missing inbox row is safe to skip;
- rebuild a tenant automatically;
- mark owner state covered by a pending or failed delivery;
- claim PostgreSQL runtime reconciliation evidence.

Those actions require a separate `FORUM-23B2G2B` commit protocol that preserves
pending inbox ordering and advances the owner cursor only after projection state
commits successfully.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
cargo test -p rustok-forum services::event::tests -- --nocapture
cargo test -p rustok-search owner_revision_tests -- --nocapture
cargo test -p rustok-server --features mod-forum host_materializes_index_query_runtime_after_source_registry -- --nocapture
node scripts/verify/verify-forum-search-owner-revision-source.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

Future PostgreSQL evidence should prove tenant isolation, legal global-sequence
gaps, exact page boundaries, unknown-event fail-safe classification and lossless
paging before G2B is allowed to mutate reconciliation state.
