# FORUM-21B idempotent topic merge owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21B adds one bounded owner workflow under the planned `FORUM-21`
umbrella: merge one active source topic into one retained active target topic
when both topics belong to the same active category.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

The owner API is:

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

## Deliberate first-merge boundary

This slice keeps the first merge transaction auditable and bounded:

- source and target must be active topics in the same active category;
- the source may contain at most 500 reply rows;
- the target retains its topic identity, translations, policy and existing reply
  positions;
- all source reply IDs move to the target in one statement;
- source reply positions shift by the target's previous maximum position;
- reply bodies, revisions, votes, mentions, quotes, attachments and parent reply
  IDs remain attached through unchanged reply IDs;
- the source topic becomes archived and locked with zero replies;
- the category retains both topic rows and therefore keeps its retained
  `topic_count`; its published `reply_count` also remains unchanged.

Cross-category merge, subscription deduplication, topic-tag union, topic vote and
read-state remapping, topic-level audience union and canonical redirects remain
separate policies. The source topic identity is retained in archived,
redirect-ready form so those later slices can migrate relations without losing
history.

## Idempotency

`operation_id` is both the immutable command identity and the Forum-local
semantic event identity. The owner serializes merge operations per tenant before
reading an existing receipt.

- first execution performs the bounded merge and stores one receipt;
- an exact retry with the same source, target, actor and normalized reason returns
  the original result;
- retry is resolved before reading the now-archived source or moved replies;
- source, target, actor or reason drift under the same operation ID fails with
  `FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

## Transaction sequence

The first execution performs one owner transaction:

1. acquire the tenant merge serialization lock and category-tree lifecycle lock;
2. resolve an immutable replay receipt before current owner state;
3. read source and target category identities without mutation and require the
   same category;
4. acquire the existing category counter scope followed by source and target
   topic counter scopes in deterministic UUID order;
5. row-lock source and target topics in deterministic UUID order, re-read both
   rows and reject category drift;
6. require non-deleted, non-archived topics in the same active category;
7. reject a source accepted solution until a later conflict policy exists;
8. verify any target solution still points to an approved target reply;
9. require the source reply set to remain within 500 rows and verify source and
   target approved-reply counters against authoritative rows;
10. compute a checked position offset from the target maximum;
11. move every source reply to the target and shift its position in one bounded
    statement;
12. update the target reply counter and archive and lock the source with zero
    replies while preserving category retained-row counters;
13. validate the retained target audience composition;
14. append one `forum.topic.merged` journal record and one immutable operation
    receipt;
15. publish source-topic, target-topic and category projection invalidations in
    that order;
16. commit.

The counter scopes are the same scopes used by normal reply and category owner
mutations. A reply creation that began first finishes before the merge counts
rows; a later reply creation waits until commit and then observes the archived
source. This prevents a reply from escaping between the source count and bulk
move without introducing a separate locking protocol.

`category.topic_count` is a retained non-deleted-row counter in the existing
owner model. The archived source remains a redirect-ready retained row, so merge
does not decrement that counter. A later explicit source soft-delete remains the
single owner of its eventual decrement.

Any error rolls back reply ownership, positions, counters, topic lifecycle,
journal event, receipt and invalidations.

## Accepted-solution boundary

A valid target accepted solution is preserved. A source accepted solution is
rejected before any mutation. Moving or choosing between source and target
solutions changes Q&A semantics and therefore requires a separate explicit
policy rather than an implicit winner.

## Persistence

`forum_topic_merge_operations` is append-only on PostgreSQL and SQLite.
Tenant-composite foreign keys bind each receipt to its source topic, retained
target topic, category and human actor. Database checks require different topic
identities, bounded reason and reply counts, non-negative position offset and
`event_id = operation_id`.

The semantic event is Forum-local:

```text
forum.topic.merged / schema version 1
```

The existing projection invalidation event remains the durable cross-consumer
repair signal; this slice does not change the shared `rustok-events` wire
catalog.

## Regression coverage

`topic_merge_sqlite` is source-ready to verify:

- target identity and existing target reply position remain unchanged;
- source reply IDs and parent links survive while positions move after the target
  maximum;
- target/source reply counters and lifecycle change exactly once while category
  retained topic and published reply counters remain unchanged;
- one immutable receipt and one matching semantic event are stored;
- exactly three new projection invalidations target source, target and category;
- exact replay is side-effect free;
- command drift fails with the typed conflict;
- cross-category merge and source accepted solution fail without partial state;
- direct receipt update and deletion are rejected.

## Remaining FORUM-21 work

The canonical `FORUM-21` entry remains `planned`. Later slices still need:

- maintainer execution of the FORUM-21A and FORUM-21B verifiers and regressions;
- public/admin transport composition and explicit authorization context;
- canonical aliases, redirects and route tombstones;
- subscription/read-state/tag/vote/audience relation merge policy;
- source/target accepted-solution conflict policy;
- cross-category merge and checked category ownership policy;
- split, fork and reply-range workflows;
- PostgreSQL concurrency evidence and retained runtime artifacts.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
