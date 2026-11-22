# forum 21s reply range move owner

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-reply-range-move-owner.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21s reply range move owner`.
- Verifier: `scripts/verify/verify-forum-reply-range-move-owner.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- # FORUM-21S bounded reply-range move owner
- `source_ready_maintainer_execution_pending`
- crates/modules/rustok-forum/contracts/forum-reply-range-move-owner.json
- a selected reply whose parent remains outside the range is detached
- an unselected child may not remain in source
- next_reply_position
- target_end_position + 1
- FORUM_TOPIC_REPLY_RANGE_MOVE_SOLUTION_CONFLICT
- No command above was run by the implementation agent
- # FORUM-21T reply-range move GraphQL transport
- moveForumTopicReplyRange
- forum_topics:manage
- ForumReplyRangeMoveService::move_reply_range
- immutable owner receipt
- FORUM-21` entry remains `planned`
- forum.topic.reply_range_moved
