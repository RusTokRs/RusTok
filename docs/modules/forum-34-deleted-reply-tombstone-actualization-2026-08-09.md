# FORUM-34P deleted-reply tombstone admission actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / tombstone-persistence-open / standard-nodebb-source-gap-explicit / shared-runner-blocked`

## Fresh cursor

FORUM-34A through FORUM-34O are merged before this slice. FORUM-34O merged as `80aca195c26e2b65414e83f1615271e3940ee4f9` and added the first bounded atomic Category/Topic/non-deleted Reply owner write path.

34P started from fresh `main` `4df29ba6adddc6e4c5560956d1abee6fa539d75c` after Pages PR #3420. During preparation, `main` advanced through Commerce PR #3421 and Events/Index PR #3423 to `745b2357db84c75bc188de8c582bfb51751b9b46`; those changes do not modify Forum source. This packet records source recheck through that cursor. Immediate premerge freshness is enforced separately by requiring the feature branch to remain `behind 0`, so unrelated later mainline movement does not require rewriting historical source findings in this packet.

The canonical Forum implementation-plan ledger still carries the stale FORUM-34 `planned` row. This dated packet records the truthful continuation without replacing the large roadmap wholesale.

## Why 34P changed shape after recheck

34O intentionally rejected every prepared `ReplyStatus::Deleted` because the import facts only carried a boolean deleted flag while the owner model persists a distinct `deleted_at` timestamp and delete revision.

Before adding a timestamp field to the main NodeBB mapping, current NodeBB source was rechecked. The current post delete implementation writes the post `deleted` flag and `deleterUid`; it does not guarantee a historical deletion timestamp. Treating an invented `deletedTimestamp` field as part of the canonical NodeBB post record would therefore overstate source fidelity.

34P consequently does **not** change `NodebbPostRecord`, does not infer deletion time from post creation time, and does not use migration execution time as historical truth.

Instead, it defines a separate explicit exporter/audit enrichment contract. An operator may supply `deletedTimestamp` only when a legacy exporter, audit log, backup, or another source actually captured that fact. Standard NodeBB deleted posts without such enrichment remain intentionally blocked from exact tombstone persistence.

## Public source-enrichment contract

34P adds `import_tombstone_preparation` and re-exports it from the Forum root.

The raw optional sidecar record is:

```rust
pub struct NodebbReplyTombstoneRecord {
    pub pid: i64,
    #[serde(rename = "deletedTimestamp", alias = "deleted_timestamp")]
    pub deleted_at_ms: i64,
}
```

`NodebbForumReplyTombstoneMapper::map_batch(...)` maps each sidecar row onto the same external identity namespace used by FORUM-34A:

`nodebb / Post / post:<pid>`.

The mapper is bounded by `MAX_FORUM_IMPORT_REPLY_TOMBSTONES_PER_BATCH = 512` and rejects:

- non-positive post IDs;
- duplicate post IDs;
- negative deletion timestamps;
- timestamps outside the owner `DateTime<Utc>` range.

It never converts a NodeBB pid into a RusTok UUID.

## Admission over the prepared relation batch

The second stage accepts the existing FORUM-34M prepared relation batch plus the optional tombstone facts:

```rust
pub struct ForumImportTombstonePreparationRequest {
    pub relations: ForumPreparedImportRelationBatch,
    pub deleted_replies: Vec<ForumImportReplyTombstoneFact>,
}
```

and returns:

```rust
pub struct ForumPreparedImportTombstoneBatch {
    pub relations: ForumPreparedImportRelationBatch,
    pub deleted_replies: Vec<ForumPreparedDeletedReplyTombstone>,
}
```

Each prepared tombstone contains only the already-admitted external Post source, already-admitted Reply UUID, and exact source-owned `deleted_at_ms`.

The original 34M relation batch is preserved unchanged inside the wrapper.

## Fail-closed rules

The tombstone preparer independently rechecks the public in-process boundary rather than assuming the caller executed every predecessor immediately beforehand.

It requires:

- at most 512 tombstone facts;
- every prepared reply source to be exact NodeBB `Post` with canonical `post:<positive id>` key;
- relation count to match prepared reply count;
- each relation source/target/locale to align with its prepared reply;
- prepared reply source uniqueness;
- tombstone source uniqueness;
- each tombstone timestamp to be non-negative and inside the owner timestamp range;
- every `ReplyStatus::Deleted` reply to have exactly one tombstone;
- every non-deleted reply to have no tombstone;
- every tombstone to match one prepared reply;
- `deleted_at_ms >= created_at_ms` for the matched reply.

Prepared tombstones preserve prepared-reply source order. No cross-batch lookup is performed.

## What 34P deliberately does not do

34P is side-effect-free. It performs no:

- database access;
- SeaORM operation;
- transaction begin/commit;
- owner reply mutation;
- revision insertion/update;
- UUID generation;
- authorization or `SecurityContext` lookup;
- event publication;
- projection invalidation.

The 34O atomic writer remains unchanged and therefore still rejects `ReplyStatus::Deleted`. This separation is intentional: source admission is closed first; owner persistence will consume only an already-admitted tombstone wrapper in the next slice.

## Owner persistence recheck for the next slice

Current owner soft-delete uses a separate `forum_replies.deleted_at` column. The normal interactive owner path marks it with `CURRENT_TIMESTAMP` and database triggers capture a `forum_reply_revisions` row with `revision_reason = 'delete'`.

That normal command cannot be reused unchanged for historical import because both the tombstone and trigger-created revision time would reflect execution time rather than the admitted historical timestamp.

The next owner primitive should therefore remain migration-specific but owner-local:

1. insert the admitted Reply/body/relations inside the existing 34O transaction;
2. keep Deleted replies out of Approved public counters and `ForumTopicReplied` events;
3. set exact admitted `deleted_at` before commit through a Forum owner helper;
4. preserve one delete revision for the persisted body and stamp that revision with the same admitted historical deletion time;
5. suppress interactive mention-added notifications for content whose final imported state is deleted;
6. continue using the same all-or-nothing content transaction.

A historical child may legitimately reference a parent whose **final imported state** is deleted: interactive create forbids choosing an already-deleted parent, but existing children are not removed when a parent is later soft-deleted. The next slice must reconstruct final historical state without pretending that the child was interactively created after deletion.

## Current FORUM-34 chain

The bounded chain is now:

`34A mapping -> 34B/34C inspection -> 34K application resolution -> 34L owner-write preparation -> 34M relation admission -> 34N relation persistence bridge -> 34O atomic non-deleted content application -> 34P optional deleted-reply tombstone enrichment admission`.

Standard current NodeBB deleted posts without an independently captured deletion timestamp remain a truthful source-data limitation, not a value that Forum fills in.

## Next cursor

The next safe slice is **FORUM-34Q**: make the existing 34O owner write service consume `ForumPreparedImportTombstoneBatch`, persist admitted deleted replies with exact owner tombstone/revision time in the same atomic transaction, and keep missing tombstone enrichment fail-closed.

Durable cross-batch checkpoint/replay execution remains separate and must wait for a genuinely shared owner-data migration runner contract rather than a Forum-local journal.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-deleted-reply-tombstone-source.mjs
```
