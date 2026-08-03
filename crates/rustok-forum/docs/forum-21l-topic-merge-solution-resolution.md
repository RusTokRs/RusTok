# FORUM-21L competing accepted-solution resolution

## Status

`source_ready_maintainer_execution_pending`

FORUM-21L closes the explicit manager-decision gap left by FORUM-21H when both
the source topic and retained target topic have valid accepted solutions. It
extends the existing same-category merge owner and does not introduce a second
transaction, receipt ledger or merge event type.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json
```

Cumulative owner contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## Owner commands

The existing strict command remains unchanged:

```rust
ForumTopicMergeService::merge_topic(...)
```

It still returns `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT` when both topics have
accepted solutions. Callers cannot gain implicit target preference by omitting a
selection.

The explicit manager command is:

```rust
ForumTopicMergeService::merge_topic_resolving_solution(
    tenant_id,
    target_topic_id,
    security,
    selected_solution_reply_id,
    MergeForumTopicInput {
        operation_id,
        source_topic_id,
        reason,
    },
)
```

Both public methods delegate to one private `merge_topic_internal` transaction.
The resolution method requires the same `forum_topics:manage` authority, routed
tenant, human actor, operation ID, source, retained target and bounded reason as
the ordinary merge.

## Selection rules

Resolution is accepted only when:

- source and target each have one valid accepted solution;
- both accepted replies are approved, non-deleted and owned by their exact
  current topic;
- `selected_solution_reply_id` equals one of those exact two reply IDs.

A nil, unrelated or unnecessary selection fails before solution, reply, topic,
statistics, event, receipt or projection mutation. The explicit command cannot
be used to alter the normal no-solution, source-only or target-only policy.

## Source solution selected

The transaction:

1. acquires the existing tenant, category, topic and solution scopes;
2. validates both competing solution rows and reply authors;
3. deletes both source and target markers;
4. decrements the rejected target reply author's solution count exactly once;
5. moves all bounded source replies to the retained target;
6. inserts the selected source marker on the target with unchanged `reply_id`,
   `marked_by_user_id` and `marked_at`;
7. re-reads and validates the transferred marker;
8. completes the existing topic counters, source archive, event, receipt and
   projection invalidations in the same transaction.

## Target solution selected

The transaction deletes only the source marker, decrements the rejected source
reply author's solution count exactly once, moves the source replies and leaves
the target marker unchanged. The winning target reply, marking actor and marking
timestamp do not change.

If both accepted replies have the same author, two accepted solutions become one
and that author's aggregate count receives one exact decrement. Anonymous
rejected replies have no user-stat row and therefore no statistic mutation.

## Fail-closed statistics

Negative solution-count transitions use one atomic conditional update requiring
an existing positive count. A missing row or zero count is treated as owner-state
drift and aborts the surrounding transaction rather than silently saturating at
zero. Positive solution-count changes keep the existing owner path.

This exact decrement also hardens existing clear and delete solution transitions,
which already remove one authoritative marker and therefore must remove exactly
one corresponding statistic contribution.

## Immutable audit and replay

Ordinary merges continue to publish:

```text
forum.topic.merged / schema version 1
```

A competing-solution resolution publishes the same Forum-local event type with
schema version 2. Its payload contains the ordinary merge fields plus:

```json
{
  "solution_resolution": {
    "source_solution_reply_id": "...",
    "target_solution_reply_id": "...",
    "selected_solution_reply_id": "...",
    "rejected_solution_reply_id": "...",
    "rejected_solution_author_id": "... or null"
  }
}
```

The existing event actor and merge reason record who made the decision and why.
No shared `rustok-events` payload contract changes because this remains the
Forum-local semantic journal.

The append-only `forum_topic_merge_operations` row remains the operation receipt
and canonical source-to-target edge. Exact replay loads the receipt and semantic
event before current topic state, validates the complete schema-1 or schema-2
payload and requires the same selected reply identity. A changed selection, or
replaying a resolved operation through the ordinary command, fails with
`FORUM_TOPIC_MERGE_OPERATION_CONFLICT` and creates no side effects.

## GraphQL transport

The additive manager mutation is:

```graphql
mergeForumTopicResolvingSolution(
  tenantId: UUID
  targetTopicId: UUID!
  input: ResolveForumTopicMergeSolutionGraphqlInput!
): GqlForumTopicMergeSolutionResolution!
```

The input carries `operationId`, `sourceTopicId`,
`selectedSolutionReplyId` and `reason`. The result contains the selected reply
identity and the existing immutable `GqlForumTopicMerge` receipt projection.

The resolver requires the `forum` module, authenticated routed tenant context
and `forum_topics:manage`, then calls the same owner service. It does not inspect
solution tables, follow canonical source aliases or hydrate a topic response.
The existing `mergeForumTopic` field is unchanged and retains its strict conflict
behavior.

## Source-ready regression

`topic_merge_solution_resolution_sqlite` is source-ready to cover:

- ordinary merge still rejecting competing solutions;
- source-selected and target-selected winner paths;
- exact winning marker metadata preservation;
- exact losing-author statistic decrement;
- schema-2 immutable audit contents;
- exact replay returning one receipt and one event;
- selected-reply drift and ordinary/resolved command-shape drift;
- unrelated selection failing with no partial state.

`topic_merge_solution_resolution_graphql_contract` builds the merged Forum schema
and checks the additive field, typed input/result, routed manager context and
single private transaction owner.

## Compatibility and remaining work

FORUM-21L adds no migration, receipt column, REST route, canonical alias, native
server function, CLI command or UI. Ordinary merge input/result, GraphQL field,
event schema version 1 and projection target list remain unchanged.

The canonical `FORUM-21` entry remains `planned`. Remaining work includes
maintainer execution and PostgreSQL evidence, native/admin command composition
and UI, cross-category merge, split, fork and reply-range workflows. Localized
routes, canonical storefront URLs and slug aliases remain FORUM-24.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-solution-resolution.mjs
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
node scripts/verify/verify-forum-topic-merge-graphql-transport.mjs
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
