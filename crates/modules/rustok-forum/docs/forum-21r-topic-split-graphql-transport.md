# FORUM-21R topic split GraphQL transport

## Status

`source_ready_maintainer_execution_pending`

FORUM-21R exposes the FORUM-21P selected-reply split owner through one additive manager-only GraphQL command. The resolver is a thin transport adapter: it derives trusted tenant and actor state from schema context, checks module admission and `forum_topics:manage`, maps the wire input to `ForumTopicSplitService::split_selected_replies`, and returns the immutable owner receipt without reading owner audit tables.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-split-graphql-transport.json
```

## Command

The GraphQL field is:

```graphql
splitForumTopicReplies(
  tenantId: UUID
  sourceTopicId: UUID!
  input: SplitForumTopicRepliesGraphqlInput!
): GqlForumTopicSplit!
```

`tenantId` is assertion-only. When supplied it must equal the routed `TenantContext`; omission uses the routed tenant. A mismatched assertion fails with `PERMISSION_DENIED` before owner execution.

The input carries the complete owner command shape except the source topic identity, which remains an explicit field argument:

- `operationId`;
- `targetTopicId`;
- bounded `replyIds`;
- `locale`;
- `title`;
- optional `slug`;
- `reason`.

The owner remains authoritative for normalization, bounds, parent closure, target absence, source non-emptiness, access-policy cloning, solution transfer, counters, receipt replay and conflict detection.

## Authorization and composition

The resolver requires the `forum` module to be enabled and an authenticated `AuthContext` with `forum_topics:manage`. It builds `SecurityContext` from the authenticated user ID and exact permission snapshot, then delegates through explicit `DatabaseConnection` and `TransactionalEventBus` schema data.

The resolver does not:

- resolve canonical merged-topic aliases for a mutation;
- query `forum_topic_split_operations` or `forum_topic_split_reply_items`;
- create a topic through `TopicService`;
- move replies directly;
- duplicate owner validation or transaction logic.

Domain errors continue through `ForumGraphqlErrorExtension`, including exact replay conflicts and owner validation failures.

## Receipt

`GqlForumTopicSplit` exposes the immutable owner receipt:

- operation and event IDs;
- source, target and category IDs;
- actor and reason;
- moved total and published reply counts;
- resulting source and target published counters;
- optional transferred solution reply ID;
- split timestamp.

An exact replay returns the same owner result. The transport does not create a second idempotency record or event.

## Compatibility and remaining scope

FORUM-21R adds one GraphQL field and no migration, REST route, admin UI, storefront UI, owner method, receipt shape or semantic-event change. Existing FORUM-21P direct owner callers remain unchanged.

Public manager UI composition remains open. FORUM-21Q reply-branch fork still has no public transport, bounded reply-range movement remains open, and retained SQLite/PostgreSQL plus mounted-host runtime evidence is still required. The canonical `FORUM-21` entry remains `planned`.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-split-owner.mjs
node scripts/verify/verify-forum-topic-split-graphql-transport.mjs
cargo test -p rustok-forum graphql::topic_split_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_split_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_split_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
