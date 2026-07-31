# FORUM-23B2G2A Forum Search owner revision ledger

## Status

`source_complete_execution_pending`

This foundation slice gives Forum one tenant-scoped monotonic projection revision
owner without changing the current Search delivery or execution path. Every
canonical PostgreSQL Forum Search invalidation now receives a positive revision
and an append-only audit row in the same owner transaction as the existing
`index.reindex_requested` outbox envelope.

Search continues to consume the legacy root event exactly as before. It does not
yet receive or enforce the Forum revision. The versioned wire event, persistent
contract consumer and dual-watermark reconciliation remain the next bounded slice.

## Owner allocation

Forum owns `forum_projection_revision_counters`. Allocation uses one PostgreSQL
statement:

```sql
INSERT ... VALUES (tenant_id, 1)
ON CONFLICT (tenant_id)
DO UPDATE SET revision = revision + 1
RETURNING revision
```

The tenant primary-key conflict serializes concurrent Forum writers. A revision
is not derived from producer time, an event UUID or Search's delivered
`ingest_sequence`.

The allocation remains inside the caller's live owner transaction. A failed
mutation, outbox write or ledger append rolls back the counter increment with the
rest of the owner work.

## Durable identity binding

`TransactionalEventBus::publish_root_in_tx_with_envelope_id` is an additive API
that validates the registered root event, creates one envelope, writes it to the
canonical outbox and returns that exact envelope ID.

The Forum invalidation owner performs these ordered steps:

1. allocate the next Forum revision;
2. write the existing validated `ReindexRequested` envelope and retain its ID;
3. append `(tenant_id, revision, event_id, target_type, target_id)` to
   `forum_projection_revision_ledger`;
4. commit the mutation, invalidation, revision and ledger row together.

The same sequence is used by direct PostgreSQL owner helpers and helpers with an
injected `TransactionalEventBus`.

## Ledger boundary

The ledger primary key is `(tenant_id, revision)` and `event_id` is unique. Exact
scope validation admits only:

- `forum` with no target ID;
- `forum_category` with a category ID;
- `forum_topic` with a topic ID.

PostgreSQL triggers reject updates and deletes. The ledger is an audit and rollout
reconciliation source, not a mutable queue or a Search-owned watermark table.

## Compatibility and rollout

The legacy root event, event type and target strings remain unchanged. Existing
Search ingestion, durable inbox ordering, retries, rebuild semantics and visible
query behavior therefore continue without a second projection path.

No root event schema, sealed contract family, event digest, Search migration,
public API, dependency or `Cargo.lock` change is introduced. SQLite remains the
existing validation-only invalidation environment because background Forum Search
projection is PostgreSQL-only.

The canonical Forum and Search plans already list owner-issued revision ordering
and reconciliation as remaining work. This foundation does not mark that result
complete: the revision is not yet present on the transport event and Search does
not enforce it.

## Next slice

`FORUM-23B2G2B` should add one sealed versioned Forum projection event carrying
`revision`, `target_type` and `target_id`; start one explicit persistent Search
contract consumer; retain rolling legacy compatibility; and store both Forum
owner revision and Search ingest sequence in durable inbox/watermark state.

A late event with a lower owner revision must be stale even when it has a later
Search ingest sequence. A legacy unversioned event must remain processable during
the rollout rather than being silently discarded by a versioned watermark.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
cargo test -p rustok-outbox transactional -- --nocapture
cargo test -p rustok-forum projection_invalidation -- --nocapture
node scripts/verify/verify-forum-search-owner-revision-ledger.mjs
cargo check -p rustok-outbox --all-targets
cargo check -p rustok-forum --all-targets
cargo xtask module validate forum
```

PostgreSQL evidence should cover first allocation, concurrent allocation for one
and multiple tenants, transaction rollback after allocation, exact envelope-ID
binding, unique event identity, target constraints, update/delete rejection and
unchanged Search ingestion of the legacy invalidation.
