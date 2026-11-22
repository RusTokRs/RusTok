# forum 21p topic split owner

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-split-owner.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21p topic split owner`.
- Verifier: `scripts/verify/verify-forum-topic-split-owner.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- marker: # FORUM-21P selected-reply topic split owner
- marker: `source_ready_maintainer_execution_pending`
- marker: crates/modules/rustok-forum/contracts/forum-topic-split-owner.json
- marker: parent-closed in both directions
- marker: cannot broaden the source topic's local restrictions
- marker: No command above was run by the implementation agent
- # FORUM-21R topic split GraphQL transport
- splitForumTopicReplies
- forum_topics:manage
- ForumTopicSplitService::split_selected_replies
- immutable owner receipt
- FORUM-21` entry remains `planned`
- forum.topic.split
