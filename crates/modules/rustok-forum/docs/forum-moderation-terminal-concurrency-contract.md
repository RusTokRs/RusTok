# Forum moderation reject/remove PostgreSQL concurrency contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-forum/tests/moderation_terminal_concurrency_postgres.rs` completes the FORUM-19 PostgreSQL revision-fence matrix for terminal reply effects that mutate public accounting or tombstone/solution state:

- `RejectPublication` against an overlapping reply-body edit;
- `Remove + SetVisibility(Removed)` against an overlapping reply-body edit where the reply is also the accepted solution.

The earlier `forum-moderation-revision-concurrency-contract.md` covers permanent topic lock and reply hide. The producer-effect contract covers successful reject/remove semantics. This target exists only to prove those heavier terminal effects cannot partially apply when the reviewed revision loses a concurrent edit race.

## Deterministic overlap

Each scenario records the current Forum reply moderation revision, then opens a transaction on an independent PostgreSQL connection and edits the reply body. The production body trigger advances the reply moderation revision inside that still-open transaction.

The real `ForumModerationSubjectAdapterFactory::reply()` is invoked on another connection with a decision that still references the old reviewed revision. Before the edit is committed, the harness waits for the real `forum/apply_moderation_decision` owner receipt to exist in `processing` state. This proves the moderation attempt has crossed producer receipt admission while the content edit still owns the revision-row lock.

The adapter task must remain incomplete while that lock is held. After commit, PostgreSQL `SERIALIZABLE` processing may surface either the semantic revision conflict directly or a retryable database/serialization outcome. Both are fail-closed and neither may apply the stale effect.

A fresh decision UUID is then issued against the same old reviewed revision after the edit is committed. It must deterministically return non-retryable `forum.moderation_subject_revision_conflict`. The fresh UUID separates the subject-fence assertion from a first overlapping attempt that PostgreSQL may leave with a reclaimable processing receipt after a retryable serialization failure.

## RejectPublication race

After the edit wins, both the overlapping attempt and deterministic stale attempt must leave:

- reply status `approved` and no `deleted_at` tombstone;
- the committed edited body intact;
- topic/category/author public reply counters at one;
- reply moderation revision exactly `reviewed_revision + 1`;
- no `forum.reply.status_changed` event.

This proves a stale rejection cannot decrement public accounting or publish rejection audit after newer content has replaced what was reviewed.

## Remove + accepted solution race

The remove race starts with an approved public reply that is also the accepted solution, with author `solution_count = 1`.

After the edit wins, both stale remove attempts must leave all of the following untouched:

- reply remains `approved` with no soft-delete tombstone;
- edited body remains present;
- topic/category/author reply counters remain one;
- `forum_solutions` still contains the accepted-solution relation;
- author `solution_count` remains one;
- moderation revision reflects only the content edit;
- no deleted status event is emitted.

Those assertions make partial owner removal observable. A stale remove cannot delete the solution first, decrement counters, or create a tombstone before discovering the revision conflict.

## Boundary

The test uses production Forum migrations through `PostgresForumTestDb`, the real producer adapter, the real shared owner-operation receipt table, and independent PostgreSQL transactions. Direct SQL is limited to truthful fixture creation, the concurrent content edit and committed-state observation.

Receipt lease/reclaim behavior after retryable serialization remains owned by the generic owner-operation receipt evidence. This contract is strictly the no-partial-effect guarantee at the Forum revision fence.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_FORUM_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-forum --test moderation_terminal_concurrency_postgres -- --nocapture

node scripts/verify/verify-forum-moderation-terminal-concurrency-postgres.mjs
```

No tests, Cargo commands, Node verifiers, formatters, real PostgreSQL migrations, workflows or CI were executed while preparing this file.
