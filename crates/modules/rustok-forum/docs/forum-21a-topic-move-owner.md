# FORUM-21A idempotent topic move owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21A starts the planned FORUM-21 workflow family with one deliberately
bounded owner command: move one existing active topic from its current active
category to another active category while retaining the topic identifier and all
relations owned by that topic.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-move-owner.json
```

The owner API is:

```rust
ForumTopicMoveService::move_topic(
    tenant_id,
    topic_id,
    security,
    MoveForumTopicInput {
        operation_id,
        target_category_id,
        reason,
    },
)
```

The command requires `forum_topics:manage` and a non-nil human actor. The public
input contains no source category: the owner locks and derives current source
state inside the transaction so a caller cannot submit stale source authority.

## Idempotency contract

`operation_id` is the immutable command identity and also the Forum-local
semantic event identity. The owner takes a tenant-wide move lock before reading
an existing receipt.

- a first command performs the move and stores one receipt;
- an exact retry with the same topic, target category, actor and normalized
  reason returns the original result;
- retry does not read the topic's now-changed category as a new source, transfer
  counters again or publish more invalidations;
- any topic, target, actor or reason drift under the same operation ID fails with
  `FORUM_TOPIC_MOVE_OPERATION_CONFLICT`.

This ordering is intentional. Idempotent replay is resolved from immutable
command history before current owner state is interpreted.

## One atomic owner transaction

The first execution performs the following bounded sequence in one database
transaction:

1. serialize topic moves for the tenant;
2. lock the current non-deleted topic;
3. lock and verify the current source and requested target categories;
4. reject archived topics, archived categories and a no-op same-category target;
5. verify that any accepted solution still belongs to one approved reply in the
   topic;
6. load the target inherited category audience policy;
7. move exactly one topic count and the topic's published reply count from the
   source category to the target category using checked arithmetic;
8. update only `forum_topics.category_id` and `updated_at`;
9. load the resulting topic audience composition against the new category;
10. append one `forum.topic.moved` record to `forum_domain_events`;
11. append one immutable `forum_topic_move_operations` receipt;
12. publish projection invalidations for the topic, source category and target
    category, in that order;
13. commit.

Any failure rolls back the category FK, both counter changes, journal event,
receipt and all invalidations together. Counter inconsistencies fail closed;
the move path does not use the compatibility counter helper that clamps negative
values to zero.

## Semantic event and projection invalidation

FORUM-21A records the semantic event in the existing Forum-owned append-only
journal:

```text
forum.topic.moved / schema version 1
```

The payload contains only operation ID, topic ID, source category ID, target
category ID, published reply count and bounded reason. `event_id` equals
`operation_id`, and replay verifies that the journal record still exactly
matches the immutable receipt.

This slice intentionally does not add a new variant to the shared
`rustok-events::DomainEvent` registry. That avoids changing the published shared
wire schemas and digests before a later cross-module consumer contract exists.
The three existing `index.reindex_requested` events remain the durable
cross-consumer notification for Forum Search projection repair.

## Persistence guards

`forum_topic_move_operations` is tenant-owned and append-only in PostgreSQL and
SQLite. Tenant-composite foreign keys bind every receipt to:

- the retained topic;
- its recorded source category;
- its recorded target category;
- the human actor.

Database checks require non-nil identities, different source and target
categories, bounded reason, non-negative published reply count and
`event_id = operation_id`. Update and delete triggers reject mutation of command
history.

## Regression coverage

`topic_move_sqlite` creates real Forum categories, a topic and an approved reply,
then verifies:

- topic and published-reply counters move exactly once;
- the topic retains its identity and relations while its category changes;
- one semantic event and one immutable operation receipt are stored;
- exactly three new projection invalidations target the topic, source category
  and target category;
- exact replay returns the same result and creates no additional state or events;
- changed replay payload fails with the typed conflict;
- same-category, archived-target and foreign-target commands leave no partial
  state;
- direct receipt update and deletion are rejected.

## Compatibility and remaining work

Existing topics require no backfill. The new table is empty until a move command
is used. Topic identity, translations, replies, accepted solution, tags,
mentions, attachments, subscriptions, votes, read state and revision rows remain
attached through the unchanged topic ID.

The canonical FORUM-21 ledger entry deliberately remains `planned` while this
slice is `source_ready_maintainer_execution_pending`. It may be promoted to
`in_progress` only after the maintainer executes both the source verifier and
the SQLite regression on one exact checkout. Later bounded slices still need:

- public/admin transport composition and authorization context;
- canonical URL aliases and redirect/tombstone behavior;
- PostgreSQL concurrency evidence;
- merge, split, fork and reply-range workflows;
- subscription deduplication and reply-position remapping for operations that
  combine or divide topic identities;
- attachment, mention, revision and audit evidence for those later workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-move-owner.mjs
cargo test -p rustok-forum --test topic_move_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
