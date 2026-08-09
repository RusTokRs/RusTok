# FORUM-34Q deleted-reply owner persistence actualization — 2026-08-10

Status: `source-ready / maintainer-execution-open / deleted-reply-owner-persistence-ready / standard-nodebb-tombstone-enrichment-conditional / shared-runner-blocked`

## Fresh cursor

FORUM-34A through FORUM-34P are merged before this slice. FORUM-34P merged as `ea3bbd6495f6826f0177f5064f583c9051bd8dea` and added the bounded, side-effect-free tombstone enrichment boundary without pretending that canonical NodeBB post state always contains a deletion timestamp.

34Q was rechecked from fresh `main` `eb946d4056db3517b464fa9cca978c9d792a8437`. The mainline commits after 34P at that cursor are Pages and Commerce changes; they do not modify Forum source. Immediate premerge freshness remains a separate `behind 0` gate so unrelated later mainline movement does not rewrite this source history.

The canonical Forum implementation-plan ledger still carries the stale FORUM-34 `planned` row. This dated packet records the truthful execution cursor without replacing the large roadmap wholesale.

## Public owner entrypoint

34Q extends the existing `ForumImportWriteService` with:

```rust
pub async fn apply_prepared_tombstone_batch(
    &self,
    security: &SecurityContext,
    batch: &ForumPreparedImportTombstoneBatch,
) -> ForumResult<ForumImportWriteResult>
```

The FORUM-34O `apply_prepared_batch(&ForumPreparedImportRelationBatch)` entrypoint is intentionally unchanged. It still rejects every `ReplyStatus::Deleted` reply, so callers cannot bypass 34P tombstone admission through the older API.

The new entrypoint re-runs `ForumImportTombstonePreparer` over the supplied wrapper and compares the re-derived prepared tombstones with the supplied identities before opening a transaction. A forged or stale wrapper therefore fails before owner writes.

It also rechecks the 34O owner boundary: bounded record count, normalized locale, tenant-wide `Manage/All` authorization for every owner kind present, relation-event mode, non-nil/unique target IDs, NodeBB source kinds, author IDs, timestamps and in-batch category/topic/reply dependencies.

## One atomic owner transaction

After side-effect-free owner content preparation, 34Q opens the same single Forum-owned transaction used by 34O.

Inside it, the service:

1. rejects already-existing admitted Category/Topic/Reply target IDs;
2. inserts categories in parent-safe order through the existing category import owner primitive;
3. inserts topics through the existing topic import owner primitive and relation bridge;
4. inserts replies in parent-safe order;
5. applies exact tombstones for final-deleted replies;
6. finalizes topic status/reply-count/last-reply facts from Approved replies only;
7. publishes the existing direct Forum projection-scope invalidation regardless of interactive event mode;
8. commits once.

Any error in tombstone capture, revision creation or revision retimestamp aborts the same transaction. No partial deleted-reply state is intentionally committed.

## Deleted reply owner primitive

The new import-only reply extension keeps the existing live-reply primitive untouched.

For a non-deleted prepared reply it delegates to the 34O `prepare_import_reply` / `insert_import_reply_in_tx` path and rejects any unexpected tombstone.

For `ReplyStatus::Deleted`, it requires the exact 34P tombstone to match both external source and admitted Reply UUID and rechecks that `deleted_at_ms >= created_at_ms` and both timestamps fit the owner time range.

It then reconstructs the final historical row by:

- preserving the admitted Reply UUID, topic, author, parent, body, locale and creation time;
- allocating the owner reply position through the existing PostgreSQL/SQLite owner allocator;
- inserting the reply in final `Deleted` status while `deleted_at` is still NULL;
- inserting the admitted body;
- materializing admitted mention/audience relations through the established 34N relation persistence path;
- forcing `SuppressAddedTargetEvents` for final-deleted content even when the batch requests domain-event emission;
- applying the exact admitted tombstone only after body/relation persistence succeeds.

The temporary NULL tombstone exists only inside the uncommitted owner transaction. It is required because the persisted Forum entity model does not expose `deleted_at` and because the established DB delete-revision triggers capture the body on the NULL -> non-NULL tombstone transition.

## Exact tombstone and delete revision time

The interactive reply delete command cannot be reused for historical import because it writes `CURRENT_TIMESTAMP`.

34Q instead performs an owner-local backend-specific update inside the existing transaction:

```text
forum_replies.deleted_at = admitted deleted_at
forum_replies.updated_at = admitted deleted_at
```

with predicates requiring the admitted tenant/reply, final `status = 'deleted'` and `deleted_at IS NULL`.

The existing PostgreSQL and SQLite soft-delete revision triggers then capture the persisted body into `forum_reply_revisions` with `revision_reason = 'delete'`.

To prevent an execution-time revision timestamp from masquerading as historical truth, the owner helper enforces:

1. zero delete revisions for that new reply before tombstoning;
2. exactly one affected reply row on the tombstone update;
3. exactly one delete revision after the trigger runs;
4. exactly one row affected when that delete revision's `created_at` is rewritten to the same admitted historical deletion timestamp.

The update is migration-specific but remains Forum-owner-local and transaction-scoped. It does not modify the schema or replace the established body-capture trigger.

## Counters, events and parent history

A final-deleted imported reply never enters Approved public counters. It does not increment topic/category/UserStats reply counts and does not contribute to the imported topic's `reply_count` or `last_reply_at` aggregate.

34Q does not fabricate interactive lifecycle events for deleted history:

- no `ForumTopicReplied` event for a final-deleted reply;
- no `ForumReplyStatusChanged` event because NodeBB tombstone enrichment does not establish the RusTok old status/moderator transition envelope;
- no mention-added events for final-deleted content.

The relation projection itself may still be materialized so owner relation state remains consistent with the stored body.

Historical child -> deleted-parent relationships are allowed in this import-only path. Interactive create correctly refuses choosing an already-deleted parent, but a historical child may have been created while the parent was live and survive after that parent was later deleted. The bounded reply graph and 34P tombstone wrapper are both revalidated before persistence.

## Deliberately unchanged

34Q does not:

- add a `deletedTimestamp` field to canonical `NodebbPostRecord`;
- infer deletion time from creation time;
- use migration execution time as historical deletion truth;
- change the normal interactive reply delete command;
- change the 34O non-deleted import entrypoint;
- modify PostgreSQL or SQLite migration definitions;
- create a second mention/quote persistence implementation;
- invent historical timestamps for relation revisions/mention rows, for which no source-owned historical fact is admitted;
- add a durable import receipt, checkpoint or replay journal;
- create a Forum-local substitute for the still-missing shared owner-data migration runner.

Standard current NodeBB deleted posts without an independently captured historical deletion timestamp remain blocked from exact deleted-reply persistence.

## Current FORUM-34 chain

The bounded in-process chain is now:

`34A mapping -> 34B/34C inspection -> 34K application resolution -> 34L owner-write preparation -> 34M relation admission -> 34N relation persistence bridge -> 34O atomic non-deleted application -> 34P exact optional tombstone admission -> 34Q atomic deleted-reply owner persistence`.

With explicit tombstone enrichment, the bounded owner transaction can now persist categories, topics, live replies and final-deleted replies while preserving admitted identities, creation times and exact deletion time.

## Next cursor

The remaining FORUM-34 execution gap is no longer the deleted-reply owner mutation itself. Durable cross-batch checkpoint/replay and operator execution must still use a genuinely shared owner-data migration runner contract rather than a Forum-local journal.

The next safe Forum slice should therefore recheck the repository for a shared migration-runner capability. If that capability still does not exist, continue only with another independently bounded source/owner contract that does not fabricate durable execution semantics.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-deleted-reply-persistence-source.mjs
```
