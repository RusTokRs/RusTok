# FORUM-21B idempotent topic merge owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21B adds one bounded owner workflow under the planned `FORUM-21`
umbrella: merge one active source topic into one retained active target topic
when both topics belong to the same active category. FORUM-21H extends that
same transaction with the accepted-solution policy described below.

The cumulative machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

The FORUM-21H policy handoff is:

```text
crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json
crates/rustok-forum/docs/forum-21h-topic-merge-solution-policy.md
```

The owner API remains:

```rust
ForumTopicMergeService::merge_topic(
    tenant_id,
    target_topic_id,
    security,
    MergeForumTopicInput {
        operation_id,
        source_topic_id,
        reason,
    },
)
```

The command requires `forum_topics:manage`, a non-nil human actor and a bounded
reason. `target_topic_id` is the retained canonical identity; the source and
target must differ.

## Bounded merge boundary

- source and target must be active topics in the same active category;
- the source may contain at most 500 reply rows;
- the target retains its topic identity, translations, policy and existing reply
  positions;
- all source reply IDs move to the target in one statement;
- source reply positions shift by the target's previous maximum position;
- reply bodies, revisions, votes, mentions, quotes, attachments and parent reply
  IDs remain attached through unchanged reply IDs;
- a source-only accepted solution follows its unchanged reply ID and preserves
  `marked_by_user_id` and `marked_at`;
- a target-only accepted solution remains unchanged;
- two accepted solutions require explicit resolution and block the merge before
  mutation;
- the source topic becomes archived and locked with zero replies;
- the category retains both topic rows and therefore keeps its retained
  `topic_count`; its published `reply_count` also remains unchanged.

Cross-category merge, canonical redirects, split, fork and reply-range workflows
remain separate policies. Subscription, read-state, tag, vote and topic-local
audience reconciliation are delivered by their dedicated FORUM-21 slices.

## Idempotency

`operation_id` is both the immutable command identity and the Forum-local
semantic event identity. The owner serializes merge operations per tenant before
reading an existing receipt.

- first execution performs the bounded merge and stores one receipt;
- an exact retry with the same source, target, actor and normalized reason returns
  the original result;
- retry is resolved before reading the now-archived source, moved replies or the
  transferred accepted solution;
- source, target, actor or reason drift under the same operation ID fails with
  `FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

## Transaction sequence

The first execution performs one owner transaction:

1. acquire the tenant merge serialization lock and category-tree lifecycle lock;
2. resolve an immutable replay receipt before current owner state;
3. read source and target category identities without mutation and require the
   same category;
4. acquire the category counter scope followed by source and target topic
   counter scopes in deterministic UUID order;
5. row-lock source and target topics in deterministic UUID order, re-read both
   rows and reject category drift;
6. require non-deleted, non-archived topics in the same active category;
7. acquire source and target solution scopes in deterministic UUID order;
8. validate every stored solution against an approved, non-deleted reply owned by
   that exact topic;
9. reject two accepted solutions with
   `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT` before any mutation;
10. require the source reply set to remain within 500 rows and verify source and
    target approved-reply counters against authoritative rows;
11. compute a checked position offset from the target maximum;
12. for a source-only solution, delete the source marker while retaining its
    reply ID, marking actor and timestamp in transaction memory;
13. move every source reply to the target and shift its position in one bounded
    statement;
14. restore a source-only marker on the target with unchanged metadata and
    revalidate it against the moved approved reply;
15. update the target reply counter and archive and lock the source with zero
    replies while preserving category retained-row counters;
16. validate retained target audience composition;
17. append one `forum.topic.merged` journal record and one immutable operation
    receipt;
18. publish source-topic, target-topic and category projection invalidations in
    that order;
19. commit.

The merge does not change `forum_user_stats.solution_count`: the accepted reply
identity and its author remain unchanged. Any error rolls back solution state,
reply ownership, positions, counters, topic lifecycle, journal event, receipt
and invalidations.

## Solution serialization and database guards

`m20260803_000016_add_forum_topic_merge_solution_policy` adds the shared
per-topic solution scope. PostgreSQL locks the affected topic rows before
advisory scope seed `31`; SQLite uses a durable topic-solution lock row under its
write transaction. Ordinary mark, replace and clear writes pass through the same
database trigger boundary, so they cannot race the merge between solution
inspection and reply movement.

PostgreSQL and SQLite also reject solution inserts or owner-key updates unless:

- the topic exists, is not deleted and is not archived;
- the reply exists in that exact tenant and topic;
- the reply is not deleted and is approved.

Delete remains available to clear a solution and to perform the source-only
transfer sequence inside the merge transaction.

## Persistence and events

`forum_topic_merge_operations` remains append-only on PostgreSQL and SQLite.
Tenant-composite foreign keys bind each receipt to its source topic, retained
target topic, category and human actor. The semantic event remains:

```text
forum.topic.merged / schema version 1
```

FORUM-21H does not change the shared event payload or receipt shape. The
existing source-topic, target-topic and category projection invalidations already
cover the solved-state change and remain the durable cross-consumer repair
signal.

## Regression coverage

`topic_merge_sqlite` is source-ready to verify:

- target identity and existing target reply positions remain unchanged;
- source reply IDs and parent links survive while positions move after the target
  maximum;
- target/source reply counters and lifecycle change exactly once while category
  retained topic and published reply counters remain unchanged;
- source-only accepted solution metadata is preserved on the retained target;
- target-only accepted solution metadata remains unchanged;
- solution author statistics receive no merge delta;
- two accepted solutions fail with the typed conflict and no partial state,
  event, receipt, counter or invalidation;
- direct solution writes cannot target a pending reply or archived topic;
- one immutable receipt and one matching semantic event are stored;
- exactly three new projection invalidations target source, target and category;
- exact replay is side-effect free;
- command drift and cross-category merge fail closed;
- direct receipt update and deletion are rejected.

## Remaining FORUM-21 work

The canonical `FORUM-21` entry remains `planned`. FORUM-21A through FORUM-21H
are bounded partial slices; remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- public/admin transport composition and exact authorization context;
- canonical aliases, redirects and route tombstones;
- an explicit manager-selected resolution command for competing solutions;
- cross-category merge and checked category ownership policy;
- split, fork and reply-range workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
