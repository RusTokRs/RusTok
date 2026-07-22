---
id: doc://crates/rustok-forum/docs/forum-12-postgres-runtime-proof.md
kind: implementation_record
language: en
status: source_ready
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-07-22
canonical_plan: doc://crates/rustok-forum/docs/implementation-plan.md
---

# FORUM-12 PostgreSQL owner runtime proof

This slice adds executable PostgreSQL coverage for the owner-write guarantees
introduced by FORUM-12D1/D2. It does not claim a successful runtime result until
the maintainer executes the test with a PostgreSQL URL.

## Concurrent D1 and D2 write

`mention_quote_runtime_postgres` creates a reply with one immutable quote, holds
its root row lock and then queues two named PostgreSQL sessions:

1. D1 explicit quote clear;
2. D2 body edit with omitted quote input.

The test observes both waits through `pg_stat_activity` instead of relying on a
sleep to establish order. After the blocker commits, D1 appends the clear
revision. D2 acquires the same owner lock, sees that its prepared relation
revision is stale and returns retryable `FORUM_RELATION_REVISION_CONFLICT`.
The test checks that the edited body rolled back, only D1 appended a revision and
the stale quote set did not return.

## Soft deletion

A reply is soft-deleted through the public `ReplyService` facade. Both the D1
replacement command and the D2 legacy body-edit facade must return
`FORUM_REPLY_DELETED`. The relation revision count and latest quote snapshot must
remain unchanged, preserving immutable discussion history.

Retention purge remains a separate operation and is not simulated by deleting
append-only relation rows in this test.

## Notifications-off profile

`PostgresForumTestDb` composes only Outbox, Taxonomy and Forum migrations. The
test creates `@moderators` through the active reply owner command without a
Notifications module or synchronous notification service. It requires one
`forum.mention.audience_added` event with schema version 1 and the same event ID
in `sys_events` and `forum_domain_events`.

This proves the producer-side degraded profile: Forum owner state and semantic
events can commit while Notifications is not composed. Consumer fan-out,
privacy filtering and target-open authorization remain under NOTIFY-03/07.

## Verification

```bash
export RUSTOK_FORUM_TEST_DATABASE_URL=postgres://...
cargo test -p rustok-forum --test mention_quote_runtime_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-forum-mention-runtime-proof.mjs
```

Tests, Cargo, verifiers and CI were not run while publishing this source-ready
slice. The canonical FORUM-12 task remains `in_progress` until successful
maintainer PostgreSQL evidence, notifications-on consumption, privacy/open
authorization and retention purge evidence are recorded.
