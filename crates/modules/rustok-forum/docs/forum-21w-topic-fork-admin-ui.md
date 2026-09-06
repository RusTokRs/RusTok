# FORUM-21W topic fork admin composition

## Status

`source_ready_maintainer_execution_pending`

FORUM-21W composes the existing FORUM-21Q reply-branch fork owner and the
FORUM-21U manager GraphQL command in both module-owned admin surfaces. It adds no
new copy policy: Leptos and Next-admin collect one source topic, one root reply,
target topic fields and a bounded reason, then submit the complete command to
`forkForumTopicReplyBranch`.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-fork-admin-ui.json
```

## Surfaces

The Leptos package mounts `ForumTopicForkAdmin` at the Forum module subpath
`fork`. The Next-admin package exposes `/dashboard/forum/fork` and contributes a
manager-only navigation item requiring `forum_topics:manage`.

Both surfaces load at most 100 topic candidates and at most 500 visible replies
for one source topic. The chosen root must be part of that loaded page. The UI
does not claim that the visible page is the complete branch: descendant
discovery, the 500-reply branch bound, topological validation and deterministic
copied identities remain owner policy.

## Retry identity

Each form owns two UUIDs:

- `operationId`, which binds the normalized owner command and immutable replay
  receipt;
- `targetTopicId`, which identifies the new topic before the command executes.

An unchanged retry retains both UUIDs. Editing the source topic, selected root,
target locale, title, slug or reason rotates both. A failed transport or owner
attempt leaves both UUIDs unchanged so the operator can retry the exact command.

## Owner boundary

The admin composition calls only the FORUM-21U GraphQL field. It does not:

- query fork operation, reply-mapping or revision-mapping audit tables;
- discover descendants or derive copied reply identities;
- copy bodies, revisions, mentions, quotes, tags or access policy;
- update source or target counters;
- copy votes, subscriptions, read state or accepted solution;
- add a native fallback or second idempotency record.

The source topic and original replies remain unchanged. The immutable receipt
shown after success comes directly from `ForumTopicForkService` and includes
copied reply, body, revision, mention and quote counts.

## Compatibility and remaining scope

FORUM-21W adds only Leptos and Next-admin composition, package-local models,
locales, documentation and a source contract. It adds no migration, REST route,
native fork command, owner method, GraphQL field, receipt shape or semantic-event
change. Direct owner and GraphQL callers remain source-compatible.

`FORUM-21` remains `planned`. Reply-range admin composition, mounted runtime and
browser evidence, complete SQLite/PostgreSQL execution evidence, and FORUM-24
localized canonical aliases remain open.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-fork-owner.mjs
node scripts/verify/verify-forum-topic-fork-graphql-transport.mjs
node scripts/verify/verify-forum-topic-fork-admin-ui.mjs
cargo test -p rustok-forum-admin topic_fork_model -- --nocapture
cargo check -p rustok-forum-admin --all-targets
npm --prefix apps/next-admin run typecheck
npm run verify:forum:admin-boundary
npm run verify:blog:forum-ui-ownership
```

No command above was run by the implementation agent, per maintainer request.
