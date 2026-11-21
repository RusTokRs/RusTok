# forum 21n topic merge admin ui

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-merge-admin-ui.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21n topic merge admin ui`.
- Verifier: `scripts/verify/verify-forum-topic-merge-admin-ui.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- marker: # FORUM-21N admin topic merge workflow
- marker: `source_ready_maintainer_execution_pending`
- marker: crates/modules/rustok-forum/contracts/forum-topic-merge-admin-ui.json
- marker: explicit GraphQL-only adapter state
- marker: never serializes that token into the client component
- marker: No command above was run by the implementation agent
- marker: # FORUM-21O native Leptos admin topic-merge transport
- marker: crates/modules/rustok-forum/contracts/forum-topic-merge-native-admin.json
- marker: direct authenticated native server-function path
- # FORUM-21B idempotent topic merge owner
- crates/modules/rustok-forum/contracts/forum-topic-merge-owner.json
- Admin composition
- mergeForumTopicResolvingSolution
- /modules/forum/merge
- /dashboard/forum/merge
