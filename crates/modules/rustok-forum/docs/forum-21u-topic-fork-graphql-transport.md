# FORUM-21U topic fork GraphQL transport

## Status

`source_ready_maintainer_execution_pending`

FORUM-21U exposes the FORUM-21Q reply-branch fork owner through one additive manager-only GraphQL command. The resolver is a thin transport adapter: it derives trusted tenant and actor state from schema context, checks module admission and `forum_topics:manage`, maps the wire input to `ForumTopicForkService::fork_reply_branch`, and returns the immutable owner receipt without reading fork operation or mapping tables.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-fork-graphql-transport.json
```

## Command

The GraphQL field is:

```graphql
forkForumTopicReplyBranch(
  tenantId: UUID
  sourceTopicId: UUID!
  input: ForkForumTopicReplyBranchGraphqlInput!
): GqlForumTopicFork!
```

`tenantId` is assertion-only. When supplied it must equal the routed `TenantContext`; omission uses the routed tenant. A mismatched assertion fails with `PERMISSION_DENIED` before owner execution.

The input carries the complete owner command shape except the source topic identity, which remains an explicit field argument:

- `operationId`;
- `targetTopicId`;
- `rootReplyId`;
- normalized `locale`;
- bounded `title`;
- optional normalized `slug`;
- bounded `reason`.

The owner remains authoritative for branch discovery, the 500-reply bound, deterministic copied identities, detached-root and copied-parent policy, source immutability, complete bounded body/revision/relation provenance, quote-target preservation, exact access/tag cloning, deliberate vote/subscription/read-state/solution non-copy policy, counters, immutable audit and replay conflict detection.

## Authorization and composition

The resolver requires the `forum` module to be enabled and an authenticated `AuthContext` with `forum_topics:manage`. It builds `SecurityContext` from the authenticated user ID and exact permission snapshot, then delegates through explicit `DatabaseConnection` and `TransactionalEventBus` schema data.

The resolver does not:

- resolve canonical merged-topic aliases for a mutation;
- query `forum_topic_fork_operations`, reply mappings or revision mappings;
- discover descendants or derive copied reply IDs;
- copy reply bodies, revisions, mentions, quotes, access policy or tags;
- update counters or source/target rows directly;
- infer an accepted solution or copy votes, subscriptions or read state;
- hydrate a localized topic after the command.

Domain errors continue through `ForumGraphqlErrorExtension`, including exact replay conflicts and owner validation failures.

## Receipt

`GqlForumTopicFork` exposes the immutable owner receipt:

- operation and event IDs;
- source and target topic IDs;
- selected root reply and category IDs;
- actor and reason;
- copied reply and approved-reply counts;
- copied body, reply-revision and relation-revision counts;
- copied mention and quote counts;
- fork timestamp.

An exact replay returns the same owner result. The transport does not create a second idempotency record, mapping set or event.

## Compatibility and remaining scope

FORUM-21U adds one GraphQL field and no migration, REST route, native command, admin UI, storefront UI, owner method, receipt shape or semantic-event change. Existing FORUM-21Q direct owner callers remain unchanged.

Public admin composition, retained runtime/browser evidence and FORUM-24 localized route work remain open. The canonical `FORUM-21` entry remains `planned`.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-fork-owner.mjs
node scripts/verify/verify-forum-topic-fork-graphql-transport.mjs
cargo test -p rustok-forum graphql::topic_fork_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_fork_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_fork_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
