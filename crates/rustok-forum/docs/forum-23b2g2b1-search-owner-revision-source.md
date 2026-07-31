# FORUM-23B2G2B1 Search owner-revision source

## Status

`source_complete_runtime_evidence_pending`

This slice exposes the append-only Forum projection revision ledger as a bounded,
transport-neutral owner-revision source for Search. The Search-owned checkpoint,
sealed versioned invalidation publisher and persistent consumer are now merged;
maintainer-executed PostgreSQL, Iggy and cross-module evidence remains open.

## Owner clock

The authoritative revision is:

```text
forum_projection_revision_ledger.revision
```

`FORUM-23B2G2A` owns this positive tenant-scoped counter and binds each committed
revision to the exact legacy `index.reindex_requested` outbox envelope identity.
Allocation, outbox persistence and ledger append occur in one owner transaction.
`FORUM-23B2G2A1` additionally enforces upgrade preflight, initial revision `1`,
exact `+1` counter updates, deferred commit-time ledger coverage and truncate
rejection. Consequently committed rows for one tenant are contiguous and ordered
from revision 1; a gap, replay or reorder is an invariant failure rather than
normal paging behavior.

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

Search owns `ForumProjectionOwnerRevisionSourcePort` and validates each page:

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

## Delivered checkpoint and versioned rollout

`FORUM-23B2G2B2` was merged through PR #2731. It adds the Search-owned owner
checkpoint, bounded tenant discovery and current-state repair protocol while
retaining the existing Forum inbox as the primary delivery lane.

`FORUM-23B2G2B3A` through `FORUM-23B2G2B3C` were merged through PRs #2738,
#2741, #2749 and #2753. They add the sealed caused typed invalidation, atomic
Forum dual publisher and default-off persistent Search consumer. All delivery
representations converge on the same legacy root envelope ID and existing
`search_projection_inbox` row.

Search `ingest_sequence` remains a delivery-order cursor owned by the durable
Search inbox. Forum `revision` is an independent owner causal clock. They are not
compared numerically and neither replaces the other.

The remaining evidence protocol is frozen by `FORUM-23B2G2B3D0`:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json
crates/rustok-forum/docs/forum-23b2g2b3d-runtime-evidence.md
```

It requires executable PostgreSQL/Iggy proof for exact duplicate recognition,
restart/acknowledgement behavior, raw and semantic poison, missing-delivery
repair, multi-process serialization, deletion/ACL ordering and Search-disabled
continuity. No runtime result is claimed by this source handoff.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
cargo test -p rustok-search owner_revision_tests -- --nocapture
cargo test -p rustok-search --test forum_projection_sweeper_contract -- --nocapture
cargo test -p rustok-search forum_contract_ingress -- --nocapture
cargo test -p rustok-server --features mod-forum host_materializes_index_query_runtime_after_source_registry -- --nocapture
node scripts/verify/verify-forum-search-owner-revision-source.mjs
node scripts/verify/verify-forum-search-owner-revision-checkpoint.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-consumer.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL evidence must cover tenant isolation, first revision, contiguous
multi-page reads, exact envelope identity, empty tail pages, invalid cursor and
limit rejection, owner checkpoint advancement only after projection success,
missing-delivery repair and the complete D0 runtime scenarios.
