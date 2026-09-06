# FORUM-21S bounded reply-range move owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21S adds the remaining copy-free reply movement owner from the canonical FORUM-21 scope. A manager can move one bounded inclusive source-position range into an existing topic while preserving reply identity and every reply-owned body, revision, relation, quote and vote row.

Machine contract:

```text
crates/rustok-forum/contracts/forum-reply-range-move-owner.json
```

## Command and replay identity

`ForumReplyRangeMoveService::move_reply_range` requires `forum_topics:manage` and a human actor. The command supplies an operation UUID, source topic through the owner argument, existing target topic UUID, inclusive positive source start/end positions and a bounded reason.

The operation ID is bound to the normalized command and explicit movement policies with SHA-256. An exact retry by the same actor returns the immutable receipt. Actor, source, target, endpoint or reason drift fails with `FORUM_TOPIC_REPLY_RANGE_MOVE_OPERATION_CONFLICT`.

## Bounded range selection

The owner selects every current source reply whose position is inside the inclusive range. Both endpoints must identify occupied source positions. The selection must contain at most 500 replies and the source must retain at least one reply.

Sparse positions inside the endpoints are allowed because previous owner operations intentionally retain historical source positions. The owner never infers a positional count from `end - start + 1`; it counts actual selected rows before writing.

The target already exists. Selected replies append after the target's current maximum position, in source-position order, and receive one contiguous target interval. The source is not compacted.

Forum allocates ordinary reply positions through the monotonic `next_reply_position` topic watermark rather than by reading only the current maximum. Migration `m20260804_000023_advance_forum_reply_range_move_positions` therefore advances the target watermark to at least `target_end_position + 1` after every moved reply update. The source watermark is never reduced, so later reply creation cannot collide with the moved interval or reuse historical source positions.

## Parent and reference policy

Parent edges use an explicit asymmetric policy:

- a selected reply whose parent remains outside the range is detached and becomes a target root;
- parent edges between selected replies keep the same reply IDs;
- each internal parent must precede its child in source position order;
- an unselected child may not remain in source after its parent moves.

The final rule prevents a cross-topic child edge and makes partial thread movement fail atomically.

Reply IDs do not change. Therefore current localized bodies, reply revisions, relation revisions, mentions, quotes, reply votes, attachments encoded in rich text, authors, statuses and timestamps remain attached to their existing identity. Quote targets and quoted revision IDs are not rewritten.

## Access and category policy

The source and target may belong to the same or different active categories, but the owner requires exact equality of:

- effective category and topic visibility layers;
- effective category and topic reply-create layers;
- topic channel access.

The command fails closed rather than reconciling or broadening target access.

Topic counts do not change. Approved reply totals are recomputed from authoritative rows for both topics. A same-category move leaves the category reply total unchanged. A cross-category move transfers exactly the moved approved contribution from the source category to the target category. User reply statistics remain unchanged because reply identity and authorship do not change.

## Accepted solution policy

An unselected source solution stays with the source topic. If the selected range contains the source solution and the target is unsolved, the existing composite reply foreign key moves the solution with the unchanged reply identity and the owner verifies the original marker metadata.

If both the moving source solution and a target solution exist, the command fails atomically with `FORUM_TOPIC_REPLY_RANGE_MOVE_SOLUTION_CONFLICT`. No winner is inferred and no solution statistic changes.

## Event and immutable audit

Migration `m20260804_000022_add_forum_reply_range_move_operations` adds PostgreSQL and SQLite storage for:

- one immutable operation receipt;
- one immutable item per moved reply, including original/target parent IDs, original/target positions and publication state;
- a SQLite tenant serialization row.

Update and delete attempts against both audit tables fail closed.

The transaction appends `forum.topic.reply_range_moved` schema 1 with `event_id == operation_id`, then publishes source/target topic projection invalidations and category invalidations for every affected category.

## Compatibility and remaining work

FORUM-21S adds no GraphQL field, REST route or admin UI and changes no existing move, merge, split or fork receipt/event. Public manager transport and admin composition remain follow-up work. Canonical localized aliases and tombstones remain under FORUM-24.

FORUM-21 remains `planned` until maintainer execution evidence and the remaining public workflows are retained.

## Maintainer verification

```bash
node scripts/verify/verify-forum-reply-range-move-owner.mjs
cargo test -p rustok-forum --test reply_range_move_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
