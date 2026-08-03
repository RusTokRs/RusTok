# FORUM-21B idempotent topic merge owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21B owns one bounded same-category merge transaction beneath the planned
`FORUM-21` umbrella. Later slices extend that same owner without adding a second
merge implementation:

- FORUM-21H — accepted-solution locking, validation and source-only transfer;
- FORUM-21I — canonical selected-read resolution through the immutable receipt;
- FORUM-21J — authorization-safe REST GET redirect for merged source IDs;
- FORUM-21K — manager-only GraphQL merge command;
- FORUM-21L — explicit manager-selected resolution when both topics have
  accepted solutions.

The cumulative machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

Focused handoffs include:

```text
crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json
crates/rustok-forum/docs/forum-21h-topic-merge-solution-policy.md
crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json
crates/rustok-forum/docs/forum-21l-topic-merge-solution-resolution.md
crates/rustok-forum/contracts/forum-topic-canonical-resolution.json
crates/rustok-forum/docs/forum-21i-topic-canonical-resolution.md
crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json
crates/rustok-forum/docs/forum-21k-topic-merge-graphql-transport.md
```

## Owner API

The ordinary command remains:

```rust
ForumTopicMergeService::merge_topic(
    tenant_id,
    target_topic_id,
    security,
    MergeForumTopicInput {
        operation_id,
        source_topic_id,
        reason,
    },
)
```

It requires `forum_topics:manage`, a non-nil human actor and a bounded reason.
`target_topic_id` is the retained identity. Source and target must be different,
active topics in the same active category.

The explicit competing-solution command is:

```rust
ForumTopicMergeService::merge_topic_resolving_solution(
    tenant_id,
    target_topic_id,
    security,
    selected_solution_reply_id,
    MergeForumTopicInput { ... },
)
```

Both methods delegate to one private `merge_topic_internal` transaction. The
explicit method does not bypass ordinary owner validation or create a second
receipt/event lane.

## Bounded merge boundary

- source and target must be active and non-deleted in one active category;
- the source contains at most 500 reply rows;
- target topic identity, translations, policy and existing reply positions stay
  authoritative;
- every source reply ID moves to the target in one statement, shifted after the
  previous target maximum;
- reply bodies, revisions, votes, mentions, quotes, attachments and parent IDs
  remain attached through unchanged reply identities;
- source becomes an archived, locked, non-deleted zero-reply tombstone;
- category retained `topic_count` and published `reply_count` remain unchanged;
- subscriptions, read states, tags, topic votes and topic-local audience state
  remain owned by their dedicated bounded reconciliation slices.

Cross-category merge, split, fork and reply ranges remain separate workflows.
Slug aliases and localized routes remain FORUM-24.

## Idempotency and canonical identity

`operation_id` is the immutable receipt identity and Forum-local semantic event
identity. The owner acquires the tenant merge lock before receipt lookup.

The exact receipt and its semantic event are validated before reading current
topic state. A retry therefore succeeds after the source has been archived and
its replies have moved. Source, target, actor, normalized reason, selected
solution or ordinary-versus-resolved command-shape drift under the same
operation ID fails with `FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

`forum_topic_merge_operations` remains append-only and is also the only
source-to-target canonical edge. Selected reads follow the bounded receipt chain
to one terminal non-deleted target. Mutations intentionally keep exact identity
semantics and do not follow merged source aliases.

## Transaction order

A first execution performs one transaction:

1. acquire tenant merge and category lifecycle serialization;
2. resolve and validate any immutable replay receipt/event;
3. read source/target categories and require one category;
4. acquire category and sorted topic counter scopes;
5. lock source and target topic rows in deterministic UUID order;
6. re-read active owner state and require the active category;
7. acquire sorted source/target solution scopes;
8. validate any solution against an approved, non-deleted reply owned by that
   exact topic;
9. build the ordinary or explicit solution plan before mutation;
10. validate source reply bounds and source/target approved-reply counters;
11. compute a checked position offset;
12. apply required solution deletion and exact losing-author statistic change;
13. move every source reply to the target;
14. reinsert and validate a selected source marker when required;
15. update target reply count and archive/lock the source;
16. validate retained target audience composition;
17. append one `forum.topic.merged` journal event and one immutable receipt;
18. publish source-topic, target-topic and category projection invalidations;
19. commit once.

Any error rolls back solution state, statistics, reply ownership, positions,
counters, topic lifecycle, event, receipt and invalidations.

## Accepted-solution policy

The ordinary outcome matrix is unchanged:

- neither topic solved — no solution mutation;
- target only — preserve target marker unchanged;
- source only — delete source marker, move replies and insert the marker on the
  target with unchanged reply ID, `marked_by_user_id` and `marked_at`;
- both solved — fail with `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT` before mutation.

FORUM-21L adds an explicit path for the final case. The selected identity must
be the exact source or target accepted reply ID.

When the source wins, both markers are deleted, the rejected target reply
author receives one exact `solution_count` decrement, replies move, and the
source marker is inserted on the target unchanged.

When the target wins, only the source marker is deleted, the rejected source
reply author receives one exact decrement, replies move, and the target marker
remains unchanged. The winner receives no statistic delta. If both replies have
the same author, two contributions become one and that author receives one
exact decrement.

Negative solution-count transitions use one atomic conditional update requiring
an existing positive contribution. Missing or zero state fails closed instead
of silently saturating at zero. Anonymous rejected replies have no user-stat
mutation.

## Solution serialization and database guards

`m20260803_000016_add_forum_topic_merge_solution_policy` remains the shared
solution boundary. PostgreSQL uses deterministic topic rows plus advisory scope
seed `31`; SQLite touches `forum_topic_solution_locks` inside its write
transaction. Mark, replace, clear, ordinary merge and explicit resolution share
that scope.

PostgreSQL and SQLite reject solution INSERT or owner-key UPDATE unless the
exact topic is active/non-deleted and the exact reply is approved, non-deleted
and owned by that tenant/topic pair. DELETE remains available for clear and
merge resolution.

## Semantic events and receipt

Ordinary merges preserve:

```text
forum.topic.merged / schema version 1
```

Explicit competing-solution resolution uses schema version 2 of the same
Forum-local event type. The ordinary payload is extended with:

```text
solution_resolution.source_solution_reply_id
solution_resolution.target_solution_reply_id
solution_resolution.selected_solution_reply_id
solution_resolution.rejected_solution_reply_id
solution_resolution.rejected_solution_author_id
```

The existing actor and reason fields record who made the decision and why. The
shared `rustok-events` catalog is unchanged. The append-only receipt row and its
schema are unchanged.

Replay validates the complete schema-1 or schema-2 payload. Selection drift or
replaying a resolved operation through the ordinary command fails with the
operation conflict and adds no solution/statistic/event/receipt/invalidation.

## Canonical reads and REST redirect

`m20260803_000017_add_forum_topic_canonical_resolution` keeps the immutable
receipt as the only canonical edge. Traversal is bounded to 32 hops and rejects
duplicate/cyclic/ambiguous history with
`FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT`.

The existing ID route behaves as follows:

- direct target GET returns the existing `200 TopicResponse`;
- merged source GET returns `308 Permanent Redirect` with tenant-relative
  `Location`;
- explicit locale is preserved and encoded;
- read authorization and target hydration occur before disclosure;
- missing/forbidden responses expose no `Location`;
- redirects use `Cache-Control: private, no-store`;
- PUT, DELETE and other commands keep exact source-ID behavior.

## GraphQL commands

FORUM-21K publishes the strict ordinary command:

```graphql
mergeForumTopic(
  tenantId: UUID
  targetTopicId: UUID!
  input: MergeForumTopicGraphqlInput!
): GqlForumTopicMerge!
```

FORUM-21L adds:

```graphql
mergeForumTopicResolvingSolution(
  tenantId: UUID
  targetTopicId: UUID!
  input: ResolveForumTopicMergeSolutionGraphqlInput!
): GqlForumTopicMergeSolutionResolution!
```

Both require the `forum` module, authenticated routed tenant context and
`forum_topics:manage`. Optional `tenantId` is assertion-only. Both call the same
owner service, return the immutable merge receipt, do not hydrate topic content
and do not follow canonical mutation aliases.

## Source-ready coverage

`topic_merge_sqlite` retains ordinary merge atomicity, source-only transfer,
target-only preservation, strict competing conflict, replay, append-only receipt
and database solution guards.

`topic_merge_solution_resolution_sqlite` covers source and target winners,
marker metadata, exact losing-author statistics, schema-2 audit, exact replay,
selection drift, command-shape drift and invalid-selection rollback.

`topic_merge_solution_resolution_graphql_contract` builds the merged schema and
checks the additive typed mutation plus one shared private transaction owner.
Canonical-resolution and Axum controller tests retain chain and redirect
coverage; FORUM-21K tests retain ordinary GraphQL composition.

## Remaining FORUM-21 work

The canonical `FORUM-21` entry remains `planned`. FORUM-21A through FORUM-21L
are bounded partial slices. Remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- native/admin merge command composition and merge/resolution UI;
- cross-category merge and checked category ownership policy;
- split, fork and reply-range workflows.

Localized routes, canonical storefront URLs, slug aliases and route tombstones
remain the planned FORUM-24 task.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
node scripts/verify/verify-forum-topic-merge-solution-resolution.mjs
node scripts/verify/verify-forum-topic-canonical-resolution.mjs
node scripts/verify/verify-forum-topic-http-redirect.mjs
node scripts/verify/verify-forum-topic-merge-graphql-transport.mjs
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_canonical_resolution_sqlite -- --nocapture
cargo test -p rustok-forum controllers::topic_redirect::tests -- --nocapture
cargo test -p rustok-forum graphql::topic_merge_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_merge_graphql_contract -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
