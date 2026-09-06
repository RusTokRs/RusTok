# FORUM-21N admin topic merge workflow

## Status

`source_ready_maintainer_execution_pending`

FORUM-21N publishes one bounded manager workflow in both module-owned admin
surfaces without creating another merge owner or transport contract. The
Leptos Forum package and the Next-admin Forum package compose the existing
`mergeForumTopic` and `mergeForumTopicResolvingSolution` mutations.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-admin-ui.json
```

## Shared owner boundary

Both surfaces require an authenticated routed tenant and
`forum_topics:manage`. The UI never supplies category authority. The existing
`ForumTopicMergeService` derives current source and target category ownership,
applies same-category or checked cross-category policy, resolves accepted
solutions, publishes the unchanged schema-version-1 event and returns the
immutable receipt.

The command does not follow a merged source alias and does not hydrate a target
topic after commit. The receipt is the result shown to the operator.

## Command policy

Each surface implements the same framework-neutral rules:

- source and retained target are different active candidates;
- the reason is trimmed, required, capped at 500 characters and rejects control
  characters;
- one UUID operation identity is generated for the command shape;
- an exact retry keeps that identity;
- changing source, target, reason or solution choice rotates the identity before
  another submission;
- when both topics are solved, the manager must explicitly keep the exact source
  or target accepted reply;
- when fewer than two solutions exist, the ordinary mutation is used and no
  implicit winner is submitted.

The selected accepted-reply identity is derived from the current candidate data,
not accepted as a free-form operator UUID.

## Leptos admin

`rustok-module.toml` registers `/modules/forum/merge`. A thin root dispatcher
keeps the existing category/topic workspace unchanged and mounts the dedicated
merge page only for that child route.

The package separation is:

```text
admin/src/topic_merge_model.rs
admin/src/transport.rs
admin/src/transport/topic_merge_graphql_adapter.rs
admin/src/ui/root.rs
admin/src/ui/topic_merge.rs
```

Candidate reads are capped at 100 active topics. The render adapter exposes
source/target selectors, bounded reason, conditional solution winner, immutable
retry identity and receipt evidence. Successful execution refreshes the
candidate projection so the archived source disappears.

This package remains in an explicit GraphQL-only adapter state. FORUM-21N does
not wrap GraphQL inside a server function, does not put an access token in a
server-function DTO and does not claim a native owner path. Direct authenticated
native server-function composition remains a separate dependency cutover and
must retain GraphQL for CSR/headless parity.

## Next-admin

The module package owns:

```text
apps/next-admin/packages/forum/src/core/topic-merge.ts
apps/next-admin/packages/forum/src/api/forum.ts
apps/next-admin/packages/forum/src/components/forum-topic-merge.tsx
apps/next-admin/packages/forum/src/locales/{en,ru}.json
```

The host page at `/dashboard/forum/merge` only resolves the authenticated
session/tenant, loads the bounded candidate list and passes public package DTOs.
Navigation is registry-driven and guarded by `forum_topics:manage`.

The server-rendered page uses the session access token only for the initial
candidate read. It never serializes that token into the client component. The
client mutation sends only the routed tenant assertion through the same-origin
`/api/rustok/graphql` proxy; the proxy restores the bearer token from the
server-side authenticated session before forwarding the GraphQL request.

Next-admin uses `crypto.randomUUID()` when available and a UUID-shaped fallback
only for older runtimes. The operation identity stays stable across a failed
request and rotates on command-shape changes. Package-owned English/Russian copy
uses the host locale selected through `next-intl`.

## Compatibility

FORUM-21N adds no migration and changes no backend Rust owner, GraphQL schema,
REST route, receipt, semantic event, canonical resolution or reconciliation
owner. Same-category, cross-category and explicit solution behavior are inherited
from FORUM-21B through FORUM-21M.

No storefront merge action is added. A source topic becomes the existing
archived canonical tombstone only after the owner transaction commits.

## Remaining FORUM-21 scope

The canonical task remains `planned`. Remaining work includes:

- direct authenticated Leptos native server-function owner composition and
  retained GraphQL parity;
- maintainer browser, GraphQL and SQLite/PostgreSQL execution evidence;
- split-selected-replies, reply-branch fork and bounded reply-range workflows;
- localized aliases and route tombstones under FORUM-24.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-admin-ui.mjs
npm run verify:forum:admin-boundary
npm run verify:blog:forum-ui-ownership
cargo test -p rustok-forum-admin topic_merge_model -- --nocapture
cargo check -p rustok-forum-admin --all-targets
npm --prefix apps/next-admin run typecheck
```

No command above was run by the implementation agent, per maintainer request.
