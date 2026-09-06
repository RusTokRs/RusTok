# FORUM-21Q reply-branch fork owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21Q adds the copy-oriented counterpart to FORUM-21P selected-reply split. A manager can create one new topic from a reply branch while leaving the source topic, replies, accepted solution and original relation history unchanged.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-fork-owner.json
```

## Command and replay identity

`ForumTopicForkService::fork_reply_branch` requires `forum_topics:manage` and a human actor. The command supplies an operation UUID, a new target-topic UUID, one source root-reply UUID, locale, title, optional slug and a bounded reason.

The normalized command is bound to the operation ID with SHA-256. An exact retry by the same actor returns the original immutable receipt. Reusing the operation ID after changing the source, target, root, locale, title, slug, reason or actor fails with `FORUM_TOPIC_FORK_OPERATION_CONFLICT`.

## Branch and reply identity

The branch is exactly the selected root plus all current descendants, with a maximum of 500 replies. The recursive query is bounded before any target row is created. Source reply positions must be topological: every descendant parent must have a lower source position than its child.

Fork is copy semantics, not move semantics:

- source reply rows and IDs remain unchanged;
- each target reply ID is deterministically derived from the operation ID and source reply ID under the `forum-topic-fork-reply-v1` SHA-256 domain;
- the copied root is detached from any source parent;
- each copied descendant points to the copied parent ID;
- target positions are assigned as `1..N` in source-position order;
- authors, statuses and reply timestamps are preserved.

This deterministic mapping is recorded in `forum_topic_fork_reply_items` and allows replay and later audit without conflating original and fork identities.

## Content and relation history

The transaction copies every bounded current localized body and all bounded reply revision history. New revision IDs are allocated and recorded in `forum_topic_fork_revision_items`.

Every Forum relation revision for a copied reply is recreated against the copied reply ID. User and moderator-audience mention projections are copied to the new relation revision without publishing duplicate mention notification events.

Quote provenance is intentionally not rewritten. A copied quote gets a new source relation revision but retains the original `quoted_id` and `quoted_revision_id`. This preserves the immutable object the author originally quoted instead of silently changing the quotation to another copy.

Attachments remain part of the copied rich-text body/revision payload. The owner does not invent a separate attachment identity or external storage mutation.

## Topic policy and deliberate non-copy scope

The new topic remains in the source category and copies the source topic's complete local shape:

- channel access;
- visibility policy and typed role/channel/group/user relations;
- reply-create policy and typed relations;
- taxonomy tag term identities.

Effective policy and tag/channel sets are reloaded and compared before the reply copy. The target cannot broaden source access.

The following state is deliberately not copied:

- topic and reply votes;
- subscriptions;
- read states;
- accepted solution.

The source accepted solution must remain valid and unchanged. The target starts unsolved even when the copied branch contains the source solution reply.

## Counters, event and audit

Approved copied replies are new persisted contributions. The transaction therefore:

- increments category topic count by one;
- increments category reply count by the copied approved count;
- sets the target topic reply count to that approved count;
- increments the manager's topic statistic;
- increments each copied approved author's reply statistic;
- leaves source counters and solution statistics unchanged.

The append-only migration `m20260804_000021_add_forum_topic_fork_operations` adds the operation receipt, source-to-target reply mappings, reply/relation revision mappings and SQLite serialization lock. Update and delete attempts against every audit table fail closed.

The transaction appends `forum.topic.forked` schema 1 with `event_id == operation_id`, then publishes target-topic and category projection invalidations. No source-topic invalidation is emitted because source state is verified unchanged.

## Compatibility and remaining work

FORUM-21Q adds no GraphQL field, REST route or admin UI and changes no existing move, merge or split receipt/event. Cross-category composition remains `fork` followed by the existing topic-move owner.

The canonical FORUM-21 task remains `planned` pending maintainer execution evidence, public manager transport/UI, bounded reply-range movement and FORUM-24 localized canonical route work.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-fork-owner.mjs
cargo test -p rustok-forum --test topic_fork_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
