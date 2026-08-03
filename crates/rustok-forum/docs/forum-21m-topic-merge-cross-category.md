# FORUM-21M checked cross-category topic merge

## Status

`source_ready_maintainer_execution_pending`

FORUM-21M extends the existing idempotent `ForumTopicMergeService` owner so the
source and retained target may belong to two different active categories. It
does not add another merge transaction, receipt table, semantic event type,
GraphQL field or canonical-resolution lane.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-cross-category.json
```

Cumulative owner contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## Owner commands

Both existing manager commands inherit the policy:

```rust
ForumTopicMergeService::merge_topic(...)
ForumTopicMergeService::merge_topic_resolving_solution(...)
```

They still require the routed tenant, `forum_topics:manage`, a non-nil human
actor, one operation ID, different source and target topic IDs, and a bounded
reason. The explicit command retains the FORUM-21L selected-solution rules.
Both commands continue to delegate to one private `merge_topic_internal`
transaction owner.

No caller submits source or target category authority. The owner reads each
current topic category before lock acquisition, locks both category counter
scopes in deterministic UUID order, locks both topic rows, and then re-reads the
topic category IDs. Category drift under the lock boundary fails before
mutation.

## Category ownership policy

Source and target categories may be the same or different, but every distinct
category must be active. The retained target topic and its category-inherited
audience policy remain authoritative after the merge.

For a same-category merge, the existing policy is unchanged:

- the source becomes an archived, locked, non-deleted tombstone;
- the target remains active;
- category `topic_count` and `reply_count` do not change;
- one category projection invalidation is emitted.

For a cross-category merge:

- the source tombstone keeps its original `category_id`;
- the target keeps its original target `category_id`;
- source and target category `topic_count` values do not change because neither
  topic row is deleted or moved between categories;
- the source topic's exact approved-reply contribution is subtracted from the
  source category `reply_count`;
- the same contribution is added to the target category `reply_count`;
- source-topic, target-topic, source-category and target-category projection
  invalidations are emitted in the owner transaction.

A zero-reply source therefore changes no category reply aggregate but still
produces both category invalidations when the categories differ.

## Checked counter transfer

The owner validates source and target topic approved-reply counts against their
stored topic counters before category mutation. A cross-category transfer then
loads both category rows under the already acquired counter scopes and requires:

- positive source and target category `topic_count` values, because each category
  owns one of the two live pre-merge topic rows;
- a non-negative moved published-reply count;
- a source category `reply_count` large enough to cover the exact moved count;
- a non-negative target category `reply_count`;
- checked addition without overflow.

The path never clamps, saturates or repairs inconsistent aggregates. Counter
underflow, negative state or overflow returns `FORUM_VALIDATION_FAILED` and
rolls back category updates, solution changes, reply movement, topic lifecycle,
semantic event, receipt, optional resolution audit and projection invalidations.

## Retained identities and relations

All existing merge bounds and identity rules remain:

- the source contains at most 500 reply rows;
- every source reply ID moves to the target in one bounded statement;
- positions shift after the previous target maximum;
- existing target positions remain unchanged;
- reply bodies, revisions, votes, mentions, quotes, attachments, parent IDs and
  accepted-solution identity remain attached through unchanged reply IDs;
- the source becomes an archived, locked, zero-reply canonical tombstone;
- the immutable receipt remains the only source-to-target edge.

Subscription, read-state, tag, topic-vote and topic-local audience reconciliation
continue through the existing bounded post-merge owners. Cross-category support
does not create a second reconciliation path.

## Accepted solutions

FORUM-21H and FORUM-21L behavior is unchanged across category boundaries:

- neither solved: no solution mutation;
- target only: preserve the target marker;
- source only: transfer the marker after its reply moves, preserving reply ID,
  marking actor and marking time;
- both solved without selection: fail with
  `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT`;
- both solved with an exact selected candidate: preserve the selected marker,
  decrement the rejected author's aggregate exactly once and append the existing
  resolution audit.

The category counter transfer happens inside the same owner transaction and is
rolled back if any later solution or merge invariant fails.

## Event and receipt compatibility

Every same-category and cross-category merge continues to append the exact
existing contract:

```text
forum.topic.merged / schema version 1
```

The payload remains the original operation/source/target/category/count/offset/
reason object. `category_id` remains the retained target category. No
`source_category_id` or cross-category extension is added.

This exact compatibility is required because subscription, read-state, tag,
vote and audience reconciliation owners validate the schema-version-1 payload
before applying their bounded repairs. The immutable
`forum_topic_merge_operations` receipt schema is also unchanged; its
`category_id` remains the retained target category.

The source category remains durably discoverable from the archived source topic
row when owner-side diagnostics require it. It is not duplicated into the event
or receipt.

## Idempotency

Receipt, semantic event and optional solution-resolution audit validation still
happen before current topic state is interpreted. An exact replay therefore
returns the original immutable receipt after the source is archived and replies
have moved.

Replay does not transfer category counters again or publish another category
invalidation. Source, target, actor, normalized reason, selected solution or
ordinary-versus-resolved command-shape drift under one operation ID continues to
fail with `FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

## GraphQL and canonical reads

The existing additive mutations need no schema change because they already
identify source and retained target topics rather than categories:

```graphql
mergeForumTopic(...)
mergeForumTopicResolvingSolution(...)
```

Both now inherit checked cross-category behavior from the same owner service.
They still use routed tenant authority, require `forum_topics:manage`, return the
immutable receipt, avoid raw category/solution/audit reads and do not follow a
merged source mutation alias.

Canonical selected reads and the authorization-safe REST `308 Permanent
Redirect` remain receipt-based and therefore work for cross-category edges
without another alias table. Localized routes, slug aliases and route tombstones
remain FORUM-24.

## Source-ready regression

`topic_merge_cross_category_sqlite` uses real Forum, Taxonomy and Outbox
migrations and owner services. It covers:

- two active categories with one source and one target topic;
- exact transfer of two published replies from the source category aggregate to
  the target category aggregate;
- unchanged category topic counts and retained source/target category IDs;
- preserved reply IDs, parent relation and deterministic shifted positions;
- exact schema-version-1 merge event with the target category and no source
  category payload extension;
- four cross-category projection invalidation targets;
- exact replay with no second aggregate transfer, event, receipt or invalidation;
- source category counter drift aborting the transaction before any partial
  merge state.

The existing same-category, accepted-solution, reconciliation, canonical-read,
REST redirect and GraphQL contract tests remain the regression baseline for
unchanged behavior.

## Compatibility and remaining work

FORUM-21M adds no migration. It changes no receipt schema, event schema,
GraphQL field, REST route, canonical resolution, solution audit or public result
type.

The canonical `FORUM-21` entry remains `planned`. Remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- native/admin merge command composition and merge/resolution UI;
- split, fork and reply-range workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-cross-category.mjs
node scripts/verify/verify-forum-topic-merge-owner.mjs
cargo test -p rustok-forum --test topic_merge_cross_category_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
