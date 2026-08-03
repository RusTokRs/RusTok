# FORUM-21L competing accepted-solution resolution

## Status

`source_ready_maintainer_execution_pending`

FORUM-21L closes the explicit manager-decision gap left by FORUM-21H when both
the source topic and retained target topic have valid accepted solutions. It
extends the existing same-category merge owner and does not introduce a second
transaction, receipt ledger or merge event contract.

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
statistics, event, receipt, audit or projection mutation. The explicit command
cannot alter the normal no-solution, source-only or target-only policy.

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
8. completes the existing topic counters and source archive;
9. appends the unchanged schema-version-1 merge event and immutable receipt;
10. appends one solution-resolution audit row linked to that receipt;
11. publishes the existing projection invalidations and commits once.

## Target solution selected

The transaction deletes only the source marker, decrements the rejected source
reply author's solution count exactly once, moves the source replies and leaves
the target marker unchanged. The winning target reply, marking actor and marking
timestamp do not change. The same receipt-linked audit records the two
candidates, selected reply and rejected reply/author.

If both accepted replies have the same author, two accepted solutions become one
and that author's aggregate count receives one exact decrement. Anonymous
rejected replies have no user-stat row and therefore no statistic mutation.

## Fail-closed statistics

Negative solution-count transitions use one atomic conditional update requiring
an existing positive count. A missing row or zero count is treated as owner-state
drift and aborts the surrounding transaction rather than silently saturating at
zero. Positive solution-count changes keep the existing owner path.

This exact decrement also hardens existing clear and delete solution transitions,
which remove one authoritative marker and therefore must remove exactly one
corresponding statistic contribution.

## Immutable audit ledger

Migration
`m20260803_000018_add_forum_topic_merge_solution_resolution` creates:

```text
forum_topic_merge_solution_resolutions
```

The primary key is `(tenant_id, operation_id)`. A tenant-composite foreign key
binds the row to the immutable merge receipt. Tenant-composite foreign keys also
bind source, target, selected and rejected reply IDs to real replies, and bind
the optional rejected author to a real user.

The row stores:

```text
source_solution_reply_id
target_solution_reply_id
selected_solution_reply_id
rejected_solution_reply_id
rejected_solution_author_id
resolved_at
```

Database checks require distinct source/target candidates and require selected
and rejected IDs to be one exact orientation of that pair. PostgreSQL and SQLite
reject UPDATE and DELETE, making the decision append-only.

The receipt and existing merge event already own the tenant, actor, source,
target, normalized reason and operation timestamp. The audit row deliberately
does not duplicate those fields.

## Merge event compatibility

Every merge, including explicit solution resolution, continues to publish the
exact existing contract:

```text
forum.topic.merged / schema version 1
```

Its payload remains the original operation/source/target/category/count/offset/
reason object and contains no `solution_resolution` extension. This is required
because subscription, read-state, tag, vote and audience reconciliation owners
validate the exact schema-version-1 merge event before repairing their state.

No shared `rustok-events` contract changes, and no post-merge reconciliation
owner needs a compatibility branch.

## Replay

The append-only `forum_topic_merge_operations` row remains the operation receipt
and canonical source-to-target edge. Exact replay loads and validates:

1. the receipt;
2. its exact schema-version-1 semantic event;
3. the optional append-only solution-resolution audit row.

The requested selection must equal the audit's selected reply. A changed
selection, or replaying a resolved operation through the ordinary command, fails
with `FORUM_TOPIC_MERGE_OPERATION_CONFLICT` and creates no side effects.

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
solution or audit tables, follow canonical source aliases or hydrate a topic
response. The existing `mergeForumTopic` field is unchanged and retains its
strict conflict behavior.

## Source-ready regression

`topic_merge_solution_resolution_sqlite` is source-ready to cover:

- ordinary merge still rejecting competing solutions;
- source-selected and target-selected winner paths;
- exact winning marker metadata preservation;
- exact losing-author statistic decrement;
- exact schema-version-1 merge event compatibility;
- append-only audit contents and update/delete rejection;
- exact replay returning one receipt, one event and one audit row;
- selected-reply drift and ordinary/resolved command-shape drift;
- unrelated selection failing with no partial state or audit.

`topic_merge_solution_resolution_graphql_contract` builds the merged Forum schema
and checks the additive field, typed input/result, routed manager context, single
private transaction owner, audit entity/migration and unchanged merge event
schema.

## Compatibility and remaining work

FORUM-21L adds one owner migration for the append-only audit ledger. It does not
change the merge receipt schema, merge event type/schema/payload, ordinary owner
method, ordinary GraphQL field, REST, canonical aliases, native server functions,
CLI commands or UI.

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
