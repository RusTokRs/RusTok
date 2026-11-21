# forum 21m topic merge cross category

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-merge-cross-category.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21m topic merge cross category`.
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
- # FORUM-21B idempotent topic merge owner
- crates/modules/rustok-forum/contracts/forum-topic-merge-owner.json
- crates/modules/rustok-forum/contracts/forum-topic-merge-admin-ui.json
- Admin composition
- mergeForumTopicResolvingSolution
- /modules/forum/merge
- /dashboard/forum/merge
