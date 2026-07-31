# FORUM-23B2G2B1 Search owner-revision source

## Status

`source_complete_consumer_checkpoint_pending`

This slice exposes the append-only Forum projection revision ledger as a bounded,
transport-neutral owner-revision source for Search. It does not change the
existing Search inbox `ingest_sequence`, create a second projection consumer or
advance a Search owner-revision checkpoint.

## Owner clock

The authoritative revision is:

```text
forum_projection_revision_ledger.revision
```

`FORUM-23B2G2A` owns this positive tenant-scoped counter and binds each committed
revision to the exact legacy `index.reindex_requested` outbox envelope identity.
Allocation, outbox persistence and ledger append occur in one owner transaction.
Consequently committed rows for one tenant are contiguous and ordered from
revision 1; a gap, replay or reorder is an invariant failure rather than normal
paging behavior.

The revision is not derived from producer time, an event UUID,
`forum_domain_events.sequence_no` or Search `ingest_sequence`.

## Forum owner boundary

`ForumEventService::list_projection_owner_revisions` reads at most 100 ledger rows
after an exclusive `after_owner_revision` cursor and orders them by revision.
The owner-facing result contains only:

- owner revision;
- exact durable invalidation envelope identity;
- registered event type `index.reindex_requested`;
- required projection impact.

Ledger target, actor, timestamps, outbox payload and ordinary Forum journal rows
remain Forum-private. The service fails closed outside PostgreSQL because the
ledger belongs to the supported PostgreSQL Forum Search runtime.

Every row requires projection reconciliation. There is no `NoProjectionChange`
classification because the ledger is written only for canonical Forum mutations
that already emitted a Search invalidation.

## Neutral Search contract

Search owns `ForumProjectionOwnerRevisionSourcePort` and validates each page
before a future checkpoint protocol may consume it:

- tenant UUID must be non-nil;
- cursor must be non-negative;
- page size must be between 1 and 100;
- returned rows must not exceed the requested page size;
- the first revision must equal `after_owner_revision + 1`;
- every later revision must be exactly the previous revision plus one;
- event IDs must be non-nil;
- event type must equal `index.reindex_requested`;
- every row must require a full reconciliation result.

Missing host composition and malformed owner pages fail closed through typed
`PortError` values. The server adapter is the only composition point between
Search and Forum; Search never queries `forum_projection_revision_ledger` or
`forum_domain_events` directly.

## Deliberate boundary with ingest ordering

Search `ingest_sequence` remains a delivery-order cursor owned by the durable
Search inbox. Forum `revision` is an independent owner causal clock. They are not
compared numerically and neither replaces the other.

This source slice registers the owner port but does not connect it to the
background sweeper. It does not:

- create or advance an owner-revision checkpoint;
- infer that an absent inbox row is safe to skip;
- rebuild a tenant automatically;
- mark owner state covered by pending or failed inbox work;
- alter existing claim, retry, completion or watermark SQL;
- claim PostgreSQL runtime reconciliation evidence.

`FORUM-23B2G2B2` should add the Search-owned checkpoint and commit protocol. It
must first drain or account for pending inbox work, rebuild when a committed owner
revision lacks durable delivery coverage, and advance the owner checkpoint only
after projection state commits successfully.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
cargo test -p rustok-search owner_revision_tests -- --nocapture
cargo test -p rustok-server --features mod-forum host_materializes_index_query_runtime_after_source_registry -- --nocapture
node scripts/verify/verify-forum-search-owner-revision-source.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL evidence should cover tenant isolation, first revision, contiguous
multi-page reads, exact envelope identity, empty tail pages, invalid cursor and
limit rejection, unavailable non-PostgreSQL behavior and host composition before
the checkpoint protocol is allowed to mutate reconciliation state.
