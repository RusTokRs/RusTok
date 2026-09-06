# FORUM-21T reply-range move GraphQL transport

## Status

`source_ready_maintainer_execution_pending`

FORUM-21T exposes the FORUM-21S bounded reply-range move owner through one additive manager-only GraphQL command. The resolver is a thin transport adapter: it derives trusted tenant and actor state from schema context, checks module admission and `forum_topics:manage`, maps the wire input to `ForumReplyRangeMoveService::move_reply_range`, and returns the immutable owner receipt without reading operation or per-reply audit tables.

Machine contract:

```text
crates/rustok-forum/contracts/forum-reply-range-move-graphql-transport.json
```

## Command

The GraphQL field is:

```graphql
moveForumTopicReplyRange(
  tenantId: UUID
  sourceTopicId: UUID!
  input: MoveForumTopicReplyRangeGraphqlInput!
): GqlForumReplyRangeMove!
```

`tenantId` is assertion-only. When supplied it must equal the routed `TenantContext`; omission uses the routed tenant. A mismatched assertion fails with `PERMISSION_DENIED` before owner execution.

The input carries the complete owner command shape except the source topic identity, which remains an explicit field argument:

- `operationId`;
- `targetTopicId`;
- inclusive `startPosition` and `endPosition`;
- bounded `reason`.

The owner remains authoritative for endpoint occupancy, the 500-reply bound, source non-emptiness, parent-edge policy, deterministic target positions, exact effective access equality, solution conflict handling, counters, category contribution, immutable audit and replay conflict detection.

## Authorization and composition

The resolver requires the `forum` module to be enabled and an authenticated `AuthContext` with `forum_topics:manage`. It builds `SecurityContext` from the authenticated user ID and exact permission snapshot, then delegates through explicit `DatabaseConnection` and `TransactionalEventBus` schema data.

The resolver does not:

- resolve canonical merged-topic aliases for a mutation;
- query `forum_reply_range_move_operations` or per-reply audit rows;
- update replies or topic/category counters directly;
- duplicate parent, ACL, solution, idempotency or transaction policy;
- hydrate a localized topic after the command.

Domain errors continue through `ForumGraphqlErrorExtension`, including exact replay conflicts, solution conflicts and owner validation failures.

## Receipt

`GqlForumReplyRangeMove` exposes the immutable owner receipt:

- operation and event IDs;
- source/target topic and category IDs;
- actor and reason;
- source and target position ranges;
- moved total and published reply counts;
- resulting source and target published counters;
- moved and resulting solution reply IDs;
- move timestamp.

An exact replay returns the same owner result. The transport does not create a second idempotency record or event.

## Compatibility and remaining scope

FORUM-21T adds one GraphQL field and no migration, REST route, admin UI, storefront UI, owner method, receipt shape or semantic-event change. Existing FORUM-21S direct owner callers remain unchanged.

Public admin composition, REST/native transports and retained runtime evidence remain open. The canonical `FORUM-21` entry remains `planned`.

## Maintainer verification

```bash
node scripts/verify/verify-forum-reply-range-move-owner.mjs
node scripts/verify/verify-forum-reply-range-move-graphql-transport.mjs
cargo test -p rustok-forum graphql::topic_reply_range_move_mutation::tests -- --nocapture
cargo test -p rustok-forum --test reply_range_move_graphql_contract -- --nocapture
cargo test -p rustok-forum --test reply_range_move_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
