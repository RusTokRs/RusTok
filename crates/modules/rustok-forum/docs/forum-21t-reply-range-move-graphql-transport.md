# forum 21t reply range move graphql transport

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-reply-range-move-graphql-transport.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21t reply range move graphql transport`.
- Verifier: `scripts/verify/verify-forum-reply-range-move-graphql-transport.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- # FORUM-21T reply-range move GraphQL transport
- `source_ready_maintainer_execution_pending`
- moveForumTopicReplyRange
- forum_topics:manage
- ForumReplyRangeMoveService::move_reply_range
- immutable owner receipt
- FORUM-21` entry remains `planned`
- No command above was run by the implementation agent
- forum.topic.reply_range_moved
