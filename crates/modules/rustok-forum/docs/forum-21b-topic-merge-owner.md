# forum 21b topic merge owner

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-merge-cross-category.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21b topic merge owner`.
- Verifier: `scripts/verify/verify-forum-topic-merge-cross-category.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- marker: # FORUM-21M checked cross-category topic merge
- marker: `source_ready_maintainer_execution_pending`
- marker: crates/modules/rustok-forum/contracts/forum-topic-merge-cross-category.json
- marker: category `topic_count` values do not change
- marker: m20260803_000019_allow_cross_category_topic_merge_redirect_edges
- marker: forum.topic.merged / schema version 1
- marker: No command above was run by the implementation agent
- marker: Cross-category category counters
- # FORUM-21I/J canonical merged-topic resolution and HTTP redirect
- One source of truth
- at most 32 edges
- FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT
- 308 Permanent Redirect
- Cache-Control: private, no-store
- FORUM-21A through FORUM-21X
- FORUM-21J
- the only source-to-target canonical edge
- Canonical merged-topic resolution and HTTP redirect
- Location: /api/forum/topics/{canonical_topic_id}
- forum_permission_denied
- FORUM-24
- [FORUM-21I/J canonical resolution and HTTP redirect]
- # FORUM-21B idempotent topic merge owner
- crates/modules/rustok-forum/contracts/forum-topic-merge-owner.json
- crates/modules/rustok-forum/contracts/forum-topic-merge-admin-ui.json
- Admin composition
- mergeForumTopicResolvingSolution
- /modules/forum/merge
- /dashboard/forum/merge
- # FORUM-21H topic merge accepted-solution policy
- FORUM-21L
- ## Explicit competing-solution resolution
- ## Append-only resolution audit
- # FORUM-21L competing accepted-solution resolution
- merge_topic_resolving_solution
- forum_topic_merge_solution_resolutions
- Resolution audit ledger
- append-only
- FORUM_TOPIC_MERGE_OPERATION_CONFLICT
- FORUM-21` entry remains `planned`
- [FORUM-21L competing solution resolution]
