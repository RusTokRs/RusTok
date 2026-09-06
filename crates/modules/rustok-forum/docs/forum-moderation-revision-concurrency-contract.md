# Forum moderation revision PostgreSQL concurrency contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-forum/tests/moderation_revision_concurrency_postgres.rs` retains real PostgreSQL concurrency evidence for the Forum-owned moderation subject revision fence.

The test uses the production Forum migrations through the shared `PostgresForumTestDb` bootstrap and materializes the real `ForumModerationSubjectAdapterFactory` against independent database connections. No test double stands in for the producer adapter.

This contract complements `forum-moderation-revision-migration-contract.md`: the migration contract proves clock initialization/backfill/trigger parity, while this target proves an edit that owns the clock concurrently cannot be silently moderated under an older reviewed revision.

## Overlap construction

Each scenario starts with a public subject whose current moderation revision is recorded as the reviewed revision. A separate PostgreSQL transaction then changes reviewed content:

- topic scenario: update the topic translation title while a permanent-lock decision still references the old topic revision;
- reply scenario: update the reply body while a hide decision still references the old reply revision.

The content update fires the production Forum moderation-revision trigger inside the still-open edit transaction. The test verifies the transaction-local revision has advanced by exactly one before starting moderation application.

The real Forum adapter is then invoked on another connection. Before allowing the edit to commit, the harness waits until the producer has created its real `forum/apply_moderation_decision` `owner_operation_receipts` row in `processing` state. This proves the moderation call has crossed the producer admission boundary while the edit still owns the revision-row lock; the concurrency assertion is not satisfied merely because Tokio delayed scheduling the application task.

While that edit transaction remains open, the moderation application must not complete.

## Safe PostgreSQL outcomes

Forum application transactions use PostgreSQL `SERIALIZABLE` isolation and lock both the active subject and its dedicated moderation-revision row. When the edit transaction wins the revision lock, PostgreSQL may expose either of two fail-closed first-call outcomes after the edit commits:

1. the adapter observes the newer revision and returns non-retryable `forum.moderation_subject_revision_conflict`; or
2. PostgreSQL resolves the serializable overlap as a retryable storage/serialization failure, surfaced as `forum.moderation_database_unavailable`.

Both are safe because neither outcome may apply the stale moderation effect.

A second application with a fresh decision UUID but the **same stale reviewed revision** is then issued after the edit has committed. The fresh UUID deliberately separates the subject-revision assertion from the first call's owner-receipt lease state if PostgreSQL chose the retryable serialization outcome. This second call must deterministically return `forum.moderation_subject_revision_conflict`.

Receipt lease/reclaim semantics for retryable producer failures are covered separately by the owner-operation receipt and lost-response evidence; this target is specifically about the subject clock fence.

## Topic lock assertions

For the topic race, after the concurrent translation edit wins:

- title equals the edited value;
- moderation revision equals `reviewed_revision + 1` exactly;
- `is_locked` remains false after both the overlapping call and deterministic stale call;
- no lock effect is allowed to retarget the edited topic silently.

## Reply hide assertions

For the reply race, after the concurrent body edit wins:

- body equals the edited value;
- moderation revision equals `reviewed_revision + 1` exactly;
- reply remains `approved`;
- topic/category public reply counters remain one;
- no `forum.reply.status_changed` event is emitted by a rejected stale moderation application.

The same invariants are rechecked after the deterministic stale call, proving the semantic conflict path is side-effect free.

## Why this is a concurrency contract

The edit transaction stays open while the real producer receipt already exists and the adapter task remains incomplete. The two operations therefore overlap on independent PostgreSQL connections and contend on the same owner revision clock. The test does not simulate concurrency by manually changing a revision after an application returns.

The retained invariant is fail-closed: a decision reviewed against revision `N` can never mutate subject state that an overlapping edit has advanced to `N+1`.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_FORUM_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-forum --test moderation_revision_concurrency_postgres -- --nocapture

node scripts/verify/verify-forum-moderation-revision-concurrency-postgres.mjs
```

No tests, Cargo commands, Node verifiers, formatters, real PostgreSQL migrations, workflows or CI were executed while preparing this file.
