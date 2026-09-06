# FORUM-21P selected-reply topic split owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21P adds the first owner-level split workflow left after the FORUM-21 move and merge chain. A manager may create one new topic in the source category and move one exact bounded set of existing replies into it without replacing reply identity or rewriting reply-owned content relations.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-split-owner.json
```

## Command and idempotency

`ForumTopicSplitService::split_selected_replies` requires `forum_topics:manage` and a human actor. The command supplies:

- one operation UUID;
- one new target-topic UUID;
- a non-empty unique set of at most 500 reply UUIDs;
- normalized locale, bounded title and optional normalized slug;
- one bounded reason.

The owner sorts the reply IDs and hashes the complete normalized command shape with SHA-256. An exact replay by the same actor returns the original immutable receipt. Reusing an operation ID after changing the source, target, reply set, locale, title, slug, reason or actor fails closed.

## Atomic selected-reply movement

The source topic and its category must remain active. The target is created in the same category, and the source must retain at least one reply. Cross-category composition is deliberately excluded from this slice: callers may compose a successful split with the already delivered topic-move owner in a later command.

The selected set is parent-closed in both directions:

- a selected child requires its parent to be selected;
- selecting a parent requires every current child in the source topic to be selected.

This prevents a `parent_reply_id` edge from crossing topic boundaries. Any violation occurs before target creation or reply mutation, so no partial split is committed.

Selected replies retain their exact IDs, authors, statuses and parent IDs. Target positions are assigned deterministically from the original source-position order as `1..N`. Source positions are not compacted because Forum persistence requires positive unique topic-local positions rather than a gap-free sequence.

Reply bodies, revisions, attachment-bearing rich text, mention projections and quote projections remain attached to the unchanged reply identity. The owner does not copy or recreate those rows.

## Access policy

Before reply movement, the owner copies the source topic's complete topic-local access shape to the new topic:

- channel access rows;
- topic visibility policy and typed role, channel, group and user relations;
- topic reply-create policy and typed role, channel, group and user relations.

Both effective policies and the channel set are reloaded and compared before movement. The target therefore inherits the same category layers and cannot broaden the source topic's local restrictions.

## Accepted solution

A valid approved accepted solution follows its unchanged reply identity when that reply is selected. Marker actor and timestamp are preserved. When the accepted reply is not selected, the solution stays with the source. No solution-author statistic changes because the accepted contribution is transferred rather than added or removed.

## Counters and audit

The transaction recomputes published reply counts from approved reply rows after movement. It updates the source and target topic counters, increments the category topic count by one, leaves the category reply count unchanged and increments the manager's topic statistic for the newly created topic.

The append-only migration `m20260803_000020_add_forum_topic_split_operations` adds:

- `forum_topic_split_locks` for SQLite serialization parity;
- `forum_topic_split_operations` as the immutable command receipt;
- `forum_topic_split_reply_items` with each reply's original and target positions plus publication state.

The transaction also appends one Forum-local `forum.topic.split` schema-1 event whose event ID equals the operation ID, then publishes source-topic, target-topic and category projection invalidations.

## Compatibility and remaining scope

FORUM-21P adds no GraphQL field, REST route or admin UI. It changes no existing move/merge receipt or event. The canonical FORUM-21 task remains `planned` pending maintainer execution evidence, a public manager transport/UI, reply-branch fork semantics, bounded range movement and the FORUM-24 localized canonical route work.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-split-owner.mjs
cargo test -p rustok-forum --test topic_split_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
