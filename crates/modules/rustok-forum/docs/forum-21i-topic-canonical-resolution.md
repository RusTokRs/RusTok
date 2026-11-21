# forum 21i topic canonical resolution

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-http-redirect.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21i topic canonical resolution`.
- Verifier: `scripts/verify/verify-forum-topic-http-redirect.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- # FORUM-21I/J canonical merged-topic resolution and HTTP redirect
- 308 Permanent Redirect
- Location: /api/forum/topics/{canonical_topic_id}
- Cache-Control: private, no-store
- forum_permission_denied
- FORUM-21A through FORUM-21X
- FORUM-24
- No command above was run by the implementation agent
- [FORUM-21I/J canonical resolution and HTTP redirect]
- `source_ready_maintainer_execution_pending`
- One source of truth
- at most 32 edges
- FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT
- Canonical merged-topic resolution and HTTP redirect
