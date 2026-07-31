# FORUM-23B2G2B2 Search owner-revision checkpoint

## Status

`source_complete_runtime_evidence_pending`

This slice connects the hardened Forum projection revision ledger to one
Search-owned reconciliation checkpoint without adding a second event consumer or
projection execution path. The existing durable Forum inbox remains the primary
delivery path and continues to execute in Search `ingest_sequence` order.

The checkpoint is an independent statement about owner causal coverage. Its
numeric value is never compared with `ingest_sequence`.

## Why tenant discovery is owner-owned

Searching only `search_projection_inbox` cannot discover a tenant whose first
Forum invalidation was committed by the owner but never reached Search. Forum
therefore exposes a second bounded method on the already composed owner-revision
port:

```text
ForumEventService::list_projection_owner_revision_tenants
ForumProjectionOwnerRevisionSourcePort::list_owner_revision_tenants
```

The owner groups `forum_projection_revision_ledger` by tenant, returns
`MAX(revision)`, orders by tenant UUID and caps one page at 256 rows. Search never
queries the Forum ledger directly and receives no target, actor, timestamp or
outbox payload.

Search persists the exclusive tenant cursor in
`search_projection_owner_scan_cursors`. Cursor updates use compare-and-set
semantics, so concurrent server workers may duplicate a bounded scan but cannot
move the shared cursor backwards. A short final page resets the cursor and starts
the next round-robin pass.

## Checkpoint storage

Migration `m20260731_000012_create_forum_owner_revision_checkpoints` adds
PostgreSQL-only Search storage:

```text
search_projection_owner_checkpoints
search_projection_owner_scan_cursors
```

A checkpoint is keyed by `(tenant_id, source_module)`, starts at revision `1`
and advances by exactly `1`. Database triggers reject a different initial value,
a skipped/replayed update, tenant/source mutation, row deletion and table
truncation. The stored event ID records the exact Forum invalidation envelope
covered by the latest revision.

The outcome is one of:

- `delivery_covered` when the exact inbox row completed or was safely skipped by
  a later Search ingest watermark;
- `rebuild_repaired` when the exact inbox row is missing or dead-lettered and a
  current-state Forum rebuild succeeds.

## Reconciliation order

Every worker cycle preserves the existing execution order:

1. drain due Forum inbox rows through the existing claim/project/retry owner;
2. recover only abandoned committed `processing` rows older than one hour;
3. page Forum owner tenant heads;
4. reconcile each tenant under the same
   `search:forum:{tenant_id}:forum` advisory lock used by the inbox claimant.

Processing recovery is bounded by both the tenant and event page limits. It
selects the oldest stale rows by `ingest_sequence`, acquires the same tenant
advisory lock and rechecks the lease predicate before updating them. A live
projector holding that lock therefore cannot be returned to `retryable_error`.

## Pending-work barrier

Owner checkpoint reconciliation stops for a tenant while any Forum inbox row is
`pending`, `processing` or `retryable_error`. It never uses an owner head to skip
durable delivery that can still complete.

Once no non-terminal work remains, Search requests the next contiguous owner
revision page and checks exact `(tenant_id, event_id)` coverage:

- `completed` and `skipped` are covered;
- a missing row or `dead_letter` requires repair;
- a non-terminal row blocks the page even if it appeared after the first barrier
  check.

This second status check closes the enqueue race without comparing owner and
ingest counters.

## Repair and commit protocol

If any revision in the bounded page lacks durable delivery coverage, Search runs
one `ForumSearchProjector::rebuild_tenant` from current owner state. One rebuild
covers the complete page because every ledger row represents a Forum projection
invalidation and the projector materializes current category/topic/reply state.

The projection transaction commits before Search attempts to advance the
checkpoint. The checkpoint transaction validates every returned event ID and
advances every owner revision in exact `+1` order. The high-water row retains the
event ID and outcome of the latest covered revision; the append-only Forum ledger
remains the complete revision-to-event history.

The projection and checkpoint are intentionally not one cross-module database
transaction. If checkpoint commit fails after a successful rebuild, the owner
revision remains unacknowledged and the next sweep repeats the idempotent
current-state rebuild. The system may perform extra safe work but cannot record
coverage before projection success.

## Rolling compatibility

The existing legacy `index.reindex_requested` event, durable inbox,
`ingest_sequence`, retry/dead-letter behavior, Search watermarks and projection
execution remain unchanged. This slice adds no root event variant, sealed event
family, event digest, public route, GraphQL/native input, storefront filter,
dependency or `Cargo.lock` change.

SQLite remains a validation-only Forum Search environment. Background inbox and
owner-checkpoint reconciliation remain PostgreSQL-only.

The historical sweeper source test and verifier still expected the pre-G1
`revision_at` ordering. They are corrected to the delivered `ingest_sequence`
order without changing runtime behavior.

## Canonical plan boundary

The Forum and Search canonical plans remain `in_progress`. Their broad
owner-revision reconciliation item is not removed in this slice because the
planned versioned owner-revision wire contract and maintainer-executed PostgreSQL
and `LINK-FORUM-03` evidence are still open.

A later bounded slice must add the versioned transport contract without creating
a second Search projection path, then execute the complete rolling-deployment,
restart, retry, deletion and ACL proof.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
cargo test -p rustok-search --test forum_projection_sweeper_contract -- --nocapture
cargo test -p rustok-search owner_revision_tests -- --nocapture
node scripts/verify/verify-forum-search-inbox-sweeper.mjs
node scripts/verify/verify-forum-search-owner-revision-source.mjs
node scripts/verify/verify-forum-search-owner-revision-checkpoint.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL evidence should cover migration upgrade/down behavior, tenant paging
and cursor wrap, concurrent cursor compare-and-set, exact event coverage,
pending/retry barriers, a first missing delivery with no inbox row, dead-letter
repair, restart recovery, an active projector older than the lease threshold,
checkpoint failure after rebuild, multi-process lock contention, deletion/ACL
projection removal and the `LINK-FORUM-03` correlation chain.
