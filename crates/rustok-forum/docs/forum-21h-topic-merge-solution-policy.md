# FORUM-21H topic merge accepted-solution policy

## Status

`source_ready_maintainer_execution_pending`

FORUM-21H established the accepted-solution subpolicy inside the bounded
FORUM-21B same-category merge owner. FORUM-21L extends the same owner with an
explicit manager-selected resolution for the previously blocked two-solution
case. Neither slice introduces a second merge transaction or receipt ledger.

Machine contracts:

```text
crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json
crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json
```

Cumulative merge contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## Why this policy is explicit

`forum_solutions` identifies one accepted reply for one topic. Moving replies
between topics changes the composite solution relation even when the reply ID is
stable. The database also requires the solution's tenant, topic and reply to
match exactly.

The merge owner cannot update reply ownership while a source solution still
points at the old topic, and it cannot silently choose between two accepted
replies without changing Q&A meaning. The ordinary merge therefore remains
fail-closed; a manager must use the explicit resolution command and name one of
the two exact accepted reply IDs.

## Outcome matrix

| Source | Retained target | Explicit selection | Outcome |
| --- | --- | --- | --- |
| no solution | no solution | none | merge without solution mutation |
| no solution | one valid solution | none | preserve the target row unchanged |
| one valid solution | no solution | none | transfer the source row to the target |
| one valid solution | one valid solution | none | fail with `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT` |
| one valid solution | one valid solution | source reply | transfer source winner, reject target |
| one valid solution | one valid solution | target reply | preserve target winner, reject source |

An explicit selection in any non-competing case fails before mutation. A stored
solution is valid only when its reply is non-deleted, approved and owned by the
exact tenant/topic relation recorded by the solution row.

## Source-only transfer

The original transfer policy remains unchanged:

1. lock source and target topic rows;
2. acquire source and target solution scopes in deterministic UUID order;
3. validate source and target solution state;
4. retain source `reply_id`, nullable `marked_by_user_id` and `marked_at` in
   transaction memory;
5. delete the source solution row;
6. move the complete bounded source reply set to the retained target;
7. insert the solution row for the target with the retained marker fields;
8. re-read and validate the transferred relation;
9. continue the existing topic counters, source archival, semantic event,
   immutable receipt and projection invalidation writes;
10. commit once.

The accepted reply and author remain unchanged, so this path has zero
`solution_count` delta.

## Competing solutions without selection

`ForumTopicMergeService::merge_topic` still returns:

```text
FORUM_TOPIC_MERGE_SOLUTION_CONFLICT
```

The conflict is detected after solution locks and validity checks but before
solution deletion, reply movement, topic mutation, statistics, event, receipt or
projection invalidation. No implicit target preference, newest-marker choice,
score heuristic or author preference is used.

## Explicit competing-solution resolution

FORUM-21L adds:

```rust
ForumTopicMergeService::merge_topic_resolving_solution(...)
```

The method delegates to the same private merge transaction as ordinary merge.
It accepts a non-nil `selected_solution_reply_id` only when both topics have
valid accepted solutions and the value equals one of those exact reply IDs.

When the source solution wins, both markers are deleted before reply movement,
the losing target reply author's statistic is decremented once, and the source
marker is reinserted on the retained target with unchanged reply ID, marking
actor and timestamp.

When the target solution wins, the source marker is deleted, the losing source
reply author's statistic is decremented once, and the target marker remains
unchanged while source replies move.

If both solutions belong to the same author, two accepted contributions become
one and the author receives one exact decrement. Anonymous losing replies have
no user-stat mutation.

## Fail-closed statistics

Negative solution-count transitions now use an atomic conditional update that
requires an existing positive count. Missing or zero state aborts the owner
transaction instead of silently saturating at zero. Positive adjustments retain
the existing owner implementation.

The exact decrement is shared by manager resolution and existing clear/delete
solution paths because every such operation removes exactly one authoritative
accepted-solution contribution.

## Shared solution mutation scope

Migration
`m20260803_000016_add_forum_topic_merge_solution_policy` remains the common
solution mutation boundary.

PostgreSQL locks affected topic rows and advisory solution scope seed `31`.
SQLite touches `forum_topic_solution_locks` inside the database write
transaction. Mark, replace, clear, ordinary merge and explicit resolution all
inspect and mutate solution state under the same scope.

PostgreSQL and SQLite continue to reject solution INSERT and owner-key UPDATE
unless the topic is active and non-deleted and the reply is approved,
non-deleted and owned by the exact tenant/topic relation.

## Immutable audit and replay

Ordinary merge preserves `forum.topic.merged` schema version 1 and its exact
payload. Explicit resolution uses schema version 2 of the same Forum-local event
type and adds an immutable `solution_resolution` object containing source,
target, selected and rejected reply IDs plus the rejected reply author ID.

The existing event actor and merge reason record who selected the winner and
why. The append-only merge receipt remains unchanged and continues to be the
operation identity and canonical redirect edge.

Exact replay validates the receipt and full semantic event before current topic
state. It requires the same selected reply. Selection drift, or replaying a
resolved operation through the ordinary command, fails with
`FORUM_TOPIC_MERGE_OPERATION_CONFLICT` and has no side effects.

## GraphQL transport

The existing `mergeForumTopic` field remains strict. FORUM-21L adds
`mergeForumTopicResolvingSolution` with typed input
`ResolveForumTopicMergeSolutionGraphqlInput` and result
`GqlForumTopicMergeSolutionResolution`.

The resolver uses routed tenant authority, requires `forum_topics:manage`, calls
the same owner service and returns the immutable merge receipt plus selected
reply identity. It does not read solution tables, hydrate topics or follow
canonical source aliases.

## Source-ready regression

Existing `topic_merge_sqlite` coverage retains ordinary transfer and strict
conflict behavior. `topic_merge_solution_resolution_sqlite` adds source-winner,
target-winner, exact statistics, schema-2 audit, replay and invalid-selection
atomicity. `topic_merge_solution_resolution_graphql_contract` verifies the
additive schema and shared owner composition.

## Remaining scope

FORUM-21 remains `planned`. Remaining work includes maintainer execution and
PostgreSQL evidence, native/admin command composition and UI, cross-category
merge, split, fork and reply-range workflows. Canonical aliases and localized
routes remain FORUM-24.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
node scripts/verify/verify-forum-topic-merge-solution-resolution.mjs
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_graphql_contract -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
