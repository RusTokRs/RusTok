# FORUM-34O atomic prepared import content actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / deleted-tombstone-admission-open / shared-runner-blocked`

## Cursor and fresh recheck

FORUM-34A through FORUM-34N are merged before this slice. 34O started from `b821161ab07bf45d3c8c57d4ff44d5935b27a464` after Commerce-only PR #3416. While the slice was being prepared, `main` first advanced through Pages-only PR #3417 and then through the Alloy/module-control-plane merge plus Commerce PR #3418 to `4699fee7c4aa820f8956e097eebf7eabbb14f3c8`. None of those commits changes `crates/rustok-forum` source. The working branch is rebased onto the final fresh `main` before PR review.

The canonical Forum implementation-plan ledger still carries the stale FORUM-34 `planned` cursor. This dated packet records the truthful source cursor without replacing that large roadmap wholesale.

### Fresh shared-runner recheck

The new Alloy work adds `AlloyReleaseImporter` under `crates/alloy/src/runner/import.rs`, so the earlier assumption about runner availability was rechecked rather than carried forward blindly.

That importer is explicitly scoped to one exact eligible **published Rhai release**: it loads immutable release/workspace bytes, imports an Alloy draft through `ScriptRegistry`, and owns Alloy-specific draft lineage/idempotency conflicts. It is not a neutral owner-data migration runner or a generic checkpoint/replay contract that Forum can implement.

FORUM-34 therefore still must not invent a Forum-local durable runner. The owner write boundary in 34O remains useful independently; durable cross-batch execution stays blocked on a genuinely shared migration runner contract.

## What 34O closes

34N made admitted mention/audience relation persistence available inside a caller-owned transaction. 34O adds the smallest Forum-owned content primitives and composes them into one bounded atomic application service.

Public owner surface:

```rust
pub const MAX_FORUM_IMPORT_APPLY_RECORDS_PER_BATCH: usize = 512;

pub struct ForumImportWriteService { /* owner DB + event bus */ }

impl ForumImportWriteService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self;

    pub async fn apply_prepared_batch(
        &self,
        security: &SecurityContext,
        batch: &ForumPreparedImportRelationBatch,
    ) -> ForumResult<ForumImportWriteResult>;
}
```

`ForumImportWriteResult` only reports the entity IDs that this immediate call committed. It is explicitly **not** a durable receipt, replay identity, checkpoint or exactly-once record.

## Authorization boundary

Historical source authors are imported data, not the identity of the operator running the migration. `Own` scope therefore cannot safely authorize this operation.

For each non-empty owner kind, 34O requires exact `PermissionScope::All` for `Action::Manage`:

- categories -> `Resource::ForumCategories`;
- topics -> `Resource::ForumTopics`;
- replies -> `Resource::ForumReplies`.

The historical author UUID is never substituted as the authorization actor.

## Independent boundary revalidation

`ForumPreparedImportRelationBatch` is an in-process public DTO and can be manually constructed. The write adapter therefore does not blindly trust that 34L/34M ran immediately beforehand.

Before any mutation it independently rechecks:

- non-nil tenant ID;
- 1..=512 total category/topic/reply records;
- valid, already-normalized batch locale;
- every category/topic/reply locale equals the batch locale;
- NodeBB source namespace and exact Category/Topic/Post kinds;
- topic body source remains a NodeBB Post ref;
- optional authors remain NodeBB User refs with non-nil admitted User UUIDs;
- non-negative admitted creation timestamps;
- non-nil, unique target IDs per owner kind;
- every topic category is inside the bounded batch;
- every reply topic and optional parent is inside the bounded batch;
- relation arrays exactly align source/target/locale with every topic/reply;
- relation event mode still matches the explicit 34L write event mode.

## Insert-only semantics

34O is a single-attempt insert path, not an idempotency ledger.

Inside the transaction it checks that none of the admitted category/topic/reply target IDs already exist in their owner tables. Existing IDs fail the whole batch. Normal primary/unique constraints remain the race-time barrier.

No Forum-local retry receipt, checkpoint or replay journal is introduced.

## Category primitive

`CategoryService::insert_import_category_in_tx(...)` remains inside the existing category owner module so it can reuse private category invariants.

It:

- preserves the admitted category UUID;
- preserves admitted parent, position, moderation/icon/color and creation time;
- re-runs category name, locale and required-slug normalization;
- locks the category tree through the established owner helper;
- requires the parent row to exist before child insertion;
- shifts siblings through the established owner placement helper;
- checks the current category route key before inserting the translation.

Categories are applied in deterministic parent-before-child order. Within a parent, the adapter orders by admitted position then UUID and rejects two imported siblings claiming the same position.

The generated category-translation UUID is an owner-internal row identity. It is not the imported category entity identity and does not replace the source-admitted category UUID.

## Topic primitive

Topic preparation runs before the transaction and reuses the established owner admission code for:

- title validation;
- locale normalization;
- canonical RichText normalization/serialization;
- topic tag normalization and the existing tag-count bound;
- flex/custom-field create preparation.

Inside the transaction the primitive preserves the admitted Topic UUID, category, author, metadata decisions, pin flag and historical creation time, then reuses existing owner helpers for:

- active-topic tag locks;
- translation persistence;
- 34N relation persistence against the exact persisted RichText body;
- localized flex values;
- channel access;
- taxonomy tags;
- category topic count;
- author topic UserStats.

### Provisional topic state

The current database/owner reply-create invariant refuses insertion when a topic is not `Open` or is locked. Historical imports can legitimately contain replies under a topic whose final imported state is `Closed`, `Archived`, or locked.

34O therefore inserts each topic as `Open` and unlocked **only inside the uncommitted private transaction**. After all replies are inserted, `finalize_import_topic_in_tx` restores the exact admitted topic status and lock state before commit.

No provisional state is externally observable.

## Reply primitive

The reply owner primitive:

- preserves admitted Reply UUID, topic, parent, author, status and creation time;
- normalizes and serializes the admitted RichText;
- validates parent existence/same-topic ownership;
- uses the existing owner monotonic `allocate_reply_position_in_tx` path instead of inventing source positions;
- writes the body with an owner-internal body-row UUID;
- persists exact 34N admitted relations in the same transaction.

Parented replies are inserted parent-before-child deterministically. NodeBB mapping does not claim an owner reply-position identity, so position remains owner-allocated.

## Public reply accounting

Fresh owner and trigger recheck confirms that only `ReplyStatus::Approved` contributes to public reply accounting.

34O therefore increments topic/category/UserStats reply counts only for imported `Approved` replies. Pending, Rejected, Hidden and Flagged replies persist but do not contribute to public counts.

After all reply inserts, the topic finalizer sets:

- `reply_count` to the number of imported Approved replies for that topic;
- `last_reply_at` to the maximum historical creation timestamp of those Approved replies;
- the exact final imported topic status/lock.

Supported-database owner triggers remain the final consistency barrier.

## Event and projection semantics

`SuppressInteractiveEvents` suppresses historical topic-created, topic-replied and added-target mention events.

`EmitDomainEvents` emits the established create-style owner events only:

- `ForumTopicCreated` for imported topics;
- `ForumTopicReplied` for imported Approved replies;
- 34N added-target relation events for materialized mentions/audiences.

34O does not synthesize lifecycle-transition history that was not present in the import facts.

Forum search/projection invalidation is a consistency signal, not an optional interactive notification. The adapter always calls `publish_forum_projection_scope_direct_in_tx(...)` before commit, even when interactive events are suppressed. That invalidation is attributed to the importing operator rather than a historical author.

## Atomicity

One `apply_prepared_batch` call has one content transaction:

1. validate authorization and bounded facts;
2. run read-only topic/reply owner preparation;
3. begin one transaction;
4. fail if target entity IDs already exist;
5. insert categories parent-first;
6. insert topics with private provisional Open/unlocked state;
7. insert replies parent-first through the owner position allocator;
8. finalize exact topic state/count/last-reply facts;
9. publish required Forum projection invalidation;
10. commit once.

Any failure before commit rolls back category/topic/reply, relation, counter/UserStats, event and projection work together.

## Deleted reply boundary remains fail-closed

34L currently carries a deleted boolean/status but **does not carry the owner `deleted_at` tombstone timestamp**.

Fresh soft-delete migration and owner-path review confirms that `deleted_at` is a separate persisted owner fact. The normal owner soft-delete path sets it explicitly and captures delete revisions. Merely inserting `ReplyStatus::Deleted` does not safely reconstruct that state.

34O therefore rejects every prepared `ReplyStatus::Deleted` before opening the write transaction. It does not invent a tombstone time from `created_at`, import execution time or any other surrogate.

This means a bounded batch containing a deleted reply is rejected atomically rather than partially importing the remaining records.

## Internal generated IDs

34O never generates imported Category, Topic or Reply UUIDs. Those remain the caller-admitted identities from 34K/34L.

Owner-internal child rows such as category translations, topic translations, reply bodies and taxonomy links may continue using their existing internal identity allocation. Those IDs have never been claimed as source entity identities.

## Current FORUM-34 import chain

The bounded import chain is now:

`34A mapping -> 34B/34C inspection -> 34K identity/application resolution -> 34L owner-write preparation -> 34M relation admission -> 34N relation persistence bridge -> 34O atomic non-deleted content application`.

The remaining deleted-reply gap is explicit rather than silently degraded.

## Next FORUM-34 cursor

The next safe slice should be **FORUM-34P**: extend the NodeBB mapping/inspection/resolution/write-preparation chain with a real admitted reply tombstone/deletion timestamp, then enable owner-equivalent deleted-reply persistence and delete-revision capture without inventing historical facts.

The fresh Alloy importer is not the neutral shared migration runner FORUM-34 needs. After the tombstone gap closes, durable checkpoint/replay execution should wait for a genuinely shared owner-data migration runner contract rather than introducing a Forum-local journal.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-atomic-import-content-source.mjs
```
