# FORUM-21K topic merge GraphQL transport

## Status

`source_ready_maintainer_execution_pending`

FORUM-21K publishes the existing same-category topic merge owner as a
manager-only GraphQL command. FORUM-21L extends the same GraphQL adapter and
owner service with an explicit competing-solution resolution command. Neither
slice adds a second merge transaction.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json
```

Cumulative owner contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## Ordinary GraphQL command

```graphql
mergeForumTopic(
  tenantId: UUID
  targetTopicId: UUID!
  input: MergeForumTopicGraphqlInput!
): GqlForumTopicMerge!
```

The input contains `operationId`, `sourceTopicId` and `reason`.
`targetTopicId` is a separate required argument because it is the retained
identity. `operationId` is the immutable idempotency identity owned by
`ForumTopicMergeService`.

The result is the immutable owner receipt projection with operation/event,
source/target/category/actor identities, reason, moved counts, position offset
and merge timestamp.

The ordinary command retains strict FORUM-21H behavior: two accepted solutions
without an explicit selection return `FORUM_TOPIC_MERGE_SOLUTION_CONFLICT`
before mutation.

## Explicit solution-resolution command

FORUM-21L adds:

```graphql
mergeForumTopicResolvingSolution(
  tenantId: UUID
  targetTopicId: UUID!
  input: ResolveForumTopicMergeSolutionGraphqlInput!
): GqlForumTopicMergeSolutionResolution!
```

The input contains `operationId`, `sourceTopicId`,
`selectedSolutionReplyId` and `reason`. The result returns the selected reply ID
and the same immutable `GqlForumTopicMerge` receipt projection.

The resolver calls
`ForumTopicMergeService::merge_topic_resolving_solution`. Selection validation,
solution locking, marker mutation, exact losing-author statistic decrement,
audit and replay remain inside the owner transaction. The GraphQL adapter does
not inspect solution storage.

## Authorization and tenant scope

Both resolvers first require the `forum` module for the routed tenant and an
authenticated `AuthContext` containing `forum_topics:manage`.

Tenant authority comes from `TenantContext`. Optional `tenantId` is assertion
only: omission uses the routed tenant and a different value fails with
`PERMISSION_DENIED` before owner execution.

Each resolver converts the authenticated permission snapshot into
`SecurityContext`. The owner independently rechecks manage authority and the
human actor requirement.

## Idempotency and errors

The first accepted command stores one immutable receipt and one matching
Forum-local semantic event. Ordinary merge uses event schema version 1.
Competing-solution resolution uses schema version 2 with the immutable selected
and rejected solution audit.

Exact replay returns the same receipt. Source, target, actor, normalized reason,
selected reply or ordinary-versus-resolved command-shape drift under the same
operation ID fails with `FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

Forum domain errors flow through `ForumGraphqlErrorExtension`. Mutations do not
follow merged source identities: a stale source remains an exact owner
validation failure rather than an implicit command redirect.

## No hydration or duplicate owner logic

Neither command hydrates a localized topic after merge. Returning the immutable
receipt avoids adding visibility, locale, profile or canonical-read behavior to
a manager command.

Both fields live on one `ForumTopicMergeMutation` object and call one
`ForumTopicMergeService`. The two public owner methods converge in one private
`merge_topic_internal` transaction.

## Source-ready coverage

The existing runtime test covers ordinary permission, tenant scope, merge and
replay. `topic_merge_graphql_contract` retains the ordinary field/input/result
schema.

`topic_merge_solution_resolution_graphql_contract` verifies:

- the additive resolution field and typed input/result;
- `selectedSolutionReplyId` exposure;
- routed module/tenant/manage composition;
- the exact owner method call;
- absence of raw solution, receipt, canonical-resolution and topic-hydration
  logic in the adapter;
- one shared private transaction owner.

The owner behavior is covered by
`topic_merge_solution_resolution_sqlite`.

## Compatibility and remaining work

The ordinary field, input and result are unchanged. FORUM-21L is additive and
adds no REST, native Leptos, CLI or UI transport. It does not change the merge
receipt schema, ordinary event schema version 1, reads, redirects or projection
targets.

The canonical `FORUM-21` entry remains `planned`. Remaining work includes
maintainer execution and PostgreSQL evidence, native/admin merge composition and
UI, cross-category merge, split, fork and reply-range workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-graphql-transport.mjs
node scripts/verify/verify-forum-topic-merge-solution-resolution.mjs
cargo test -p rustok-forum graphql::topic_merge_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_merge_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
