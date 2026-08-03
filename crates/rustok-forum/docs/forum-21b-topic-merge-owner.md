# FORUM-21B idempotent topic merge owner

## Status

`source_ready_maintainer_execution_pending`

FORUM-21B owns one bounded merge transaction beneath the planned `FORUM-21`
umbrella. Later slices extend that same owner without adding a second merge
implementation:

- FORUM-21H — accepted-solution locking, validation and source-only transfer;
- FORUM-21I — canonical selected-read resolution through the immutable receipt;
- FORUM-21J — authorization-safe REST GET redirect for merged source IDs;
- FORUM-21K — manager-only GraphQL merge command;
- FORUM-21L — explicit manager-selected resolution when both topics have
  accepted solutions;
- FORUM-21M — checked same-category or cross-category ownership and category
  reply-counter transfer.

The cumulative machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

Focused handoffs include:

```text
crates/rustok-forum/contracts/forum-topic-merge-cross-category.json
crates/rustok-forum/docs/forum-21m-topic-merge-cross-category.md
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
active topics. Their current categories may be the same or different, but every
distinct category must be active.

The explicit competing-solution command remains:

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
receipt/event/counter lane.

## Bounded merge boundary

- source and target must be active and non-deleted;
- each distinct source/target category must be active;
- the source contains at most 500 reply rows;
- target topic identity, translations, category-inherited policy and existing
  reply positions stay authoritative;
- every source reply ID moves to the target in one statement, shifted after the
  previous target maximum;
- reply bodies, revisions, votes, mentions, quotes, attachments and parent IDs
  remain attached through unchanged reply identities;
- source becomes an archived, locked, non-deleted zero-reply tombstone in its
  original category;
- same-category category counters remain unchanged;
- cross-category topic counters remain unchanged while the exact moved published
  reply contribution transfers from source category to target category;
- subscriptions, read states, tags, topic votes and topic-local audience state
  remain owned by their dedicated bounded reconciliation slices.

Split, fork and reply ranges remain separate workflows. Slug aliases and
localized routes remain FORUM-24.

## Idempotency and canonical identity

`operation_id` is the immutable receipt identity and Forum-local semantic event
identity. The owner acquires the tenant merge lock before receipt lookup.

The exact receipt, its schema-version-1 event and any optional append-only
solution-resolution audit are validated before reading current topic state. A
retry therefore succeeds after the source has been archived, category reply
counters have transferred and replies have moved. Source, target, actor,
normalized reason, selected solution or ordinary-versus-resolved command-shape
drift under the same operation ID fails with
`FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

`forum_topic_merge_operations` remains append-only and is also the only
source-to-target canonical edge. Selected reads follow the bounded receipt chain
to one terminal non-deleted target. Mutations intentionally keep exact identity
semantics and do not follow merged source aliases.

## Transaction order

A first execution performs one transaction:

1. acquire tenant merge and category lifecycle serialization;
2. resolve and validate any immutable replay receipt, schema-version-1 event and
   optional resolution audit;
3. read source and target topic category ownership;
4. acquire sorted, deduplicated source/target category counter scopes followed by
   sorted topic counter scopes;
5. lock source and target topic rows in deterministic UUID order;
6. re-read active owner state, reject category drift and require every distinct
   category active;
7. acquire sorted source/target solution scopes;
8. validate any solution against an approved, non-deleted reply owned by that
   exact topic;
9. build the ordinary or explicit solution plan before mutation;
10. validate source reply bounds and source/target approved-reply counters;
11. compute a checked position offset and target reply-count sum;
12. when categories differ, transfer the exact moved published-reply contribution
    between category aggregates with checked arithmetic;
13. apply required solution deletion and exact losing-author statistic change;
14. move every source reply to the target;
15. reinsert and validate a selected source marker when required;
16. update target reply count and archive/lock the source in its original
    category;
17. validate retained target audience composition;
18. append one unchanged schema-version-1 `forum.topic.merged` journal event;
19. append one immutable merge receipt whose `category_id` is the retained target
    category;
20. append one receipt-linked solution-resolution audit when an explicit winner
    was selected;
21. publish source-topic, target-topic, source-category and, when distinct,
    target-category projection invalidations;
22. commit once.

Any error rolls back category counters, solution state, statistics, reply
ownership, positions, topic lifecycle, event, receipt, audit and invalidations.

## Cross-category category counters

FORUM-21M keeps category ownership explicit without changing topic identity.
The archived source tombstone remains in the source category and the retained
target remains in the target category. Therefore both category `topic_count`
values remain unchanged.

For a same-category merge, the category `reply_count` also remains unchanged.
For a cross-category merge, only the exact source topic approved-reply
contribution moves:

```text
source_category.reply_count -= moved_published_reply_count
target_category.reply_count += moved_published_reply_count
```

The owner requires positive source and target category topic counts,
non-negative category reply aggregates, enough source reply contribution and a
checked target addition. It never saturates or clamps inconsistent aggregates.
Underflow, negative state or overflow fails with `FORUM_VALIDATION_FAILED` before
any transaction can commit.

PostgreSQL category counter scopes are sorted and deduplicated before the sorted
topic scopes. SQLite performs the same state transition inside its serialized
write transaction. Same-category behavior uses one category scope and emits one
category invalidation; cross-category behavior uses two distinct category scopes
and emits both category invalidations.

## Accepted-solution policy

The ordinary outcome matrix is unchanged across category boundaries:

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

## Resolution audit ledger

`m20260803_000018_add_forum_topic_merge_solution_resolution` creates the
append-only `forum_topic_merge_solution_resolutions` table.

Its `(tenant_id, operation_id)` primary key is also a tenant-composite foreign
key to the immutable merge receipt. The row stores source/target candidate
reply IDs, selected and rejected reply IDs, optional rejected author and
`resolved_at`. All reply IDs and the optional author use tenant-composite foreign
keys. A database check requires selected/rejected IDs to be one exact orientation
of the source/target candidate pair. PostgreSQL and SQLite reject UPDATE and
DELETE.

The linked receipt and event already own actor, reason, source, target, retained
target category and merge time, so the audit row does not duplicate them.

## Semantic event compatibility

Every same-category and cross-category merge, including explicit solution
resolution, preserves the exact contract:

```text
forum.topic.merged / schema version 1
```

The payload remains the original operation/source/target/category/count/offset/
reason object. `category_id` remains the retained target category. It contains
no source-category or solution-resolution extension. This keeps the existing
subscription, read-state, tag, vote and audience reconciliation owners
compatible because each validates the exact schema-version-1 event before
repairing state.

The shared `rustok-events` catalog, receipt schema and existing payload shape are
unchanged. Replay validates the event and optional audit separately. Selection
or command-shape drift fails with the operation conflict and adds no
category-counter transfer, solution/statistic/event/receipt/audit/invalidation.

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

Receipt-based canonical reads require no extra cross-category alias store.
Localized routes and slug aliases remain FORUM-24.

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
and do not follow canonical mutation aliases. Because category authority is
owner-derived, both commands inherit FORUM-21M cross-category behavior without
a GraphQL schema change.

## Source-ready coverage

`topic_merge_sqlite` retains same-category atomicity, source-only transfer,
target-only preservation, strict competing conflict, replay, append-only receipt
and database solution guards.

`topic_merge_cross_category_sqlite` covers checked source/target category
ownership, unchanged category topic counts, exact published-reply aggregate
transfer, retained source and target category IDs, four projection invalidation
targets, exact schema-version-1 payload, exact replay and full rollback on source
category aggregate drift.

`topic_merge_solution_resolution_sqlite` covers source and target winners,
marker metadata, exact losing-author statistics, exact schema-version-1 merge
event payload, append-only audit contents/update-delete rejection, exact replay,
selection drift, command-shape drift and invalid-selection rollback.

`topic_merge_solution_resolution_graphql_contract` builds the merged schema and
checks the additive typed mutation, one shared private transaction owner, audit
entity/migration and unchanged merge-event schema. Canonical-resolution and
Axum controller tests retain chain and redirect coverage; FORUM-21K tests retain
ordinary GraphQL composition.

## Remaining FORUM-21 work

The canonical `FORUM-21` entry remains `planned`. FORUM-21A through FORUM-21M
are bounded partial slices. Remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- native/admin merge command composition and merge/resolution UI;
- split, fork and reply-range workflows.

Localized routes, canonical storefront URLs, slug aliases and route tombstones
remain the planned FORUM-24 task.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-cross-category.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
node scripts/verify/verify-forum-topic-merge-solution-resolution.mjs
node scripts/verify/verify-forum-topic-canonical-resolution.mjs
node scripts/verify/verify-forum-topic-http-redirect.mjs
node scripts/verify/verify-forum-topic-merge-graphql-transport.mjs
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_cross_category_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_canonical_resolution_sqlite -- --nocapture
cargo test -p rustok-forum controllers::topic_redirect::tests -- --nocapture
cargo test -p rustok-forum graphql::topic_merge_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_merge_graphql_contract -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
