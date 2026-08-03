# FORUM-21K topic merge GraphQL transport

## Status

`source_ready_maintainer_execution_pending`

FORUM-21K publishes the existing same-category topic merge owner as one
manager-only GraphQL command. It does not add a second merge implementation or
change the FORUM-21B owner transaction.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json
```

The cumulative owner contract remains:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## GraphQL contract

The mutation is:

```graphql
mergeForumTopic(
  tenantId: UUID
  targetTopicId: UUID!
  input: MergeForumTopicGraphqlInput!
): GqlForumTopicMerge!
```

The input contains:

```text
operationId
sourceTopicId
reason
```

`targetTopicId` is a separate required argument because it is the retained
identity. `operationId` is the immutable idempotency identity already owned by
`ForumTopicMergeService`.

The result is the immutable owner receipt projection:

```text
operationId
 eventId
 sourceTopicId
 targetTopicId
 categoryId
 actorId
 reason
 movedReplyCount
 movedPublishedReplyCount
 resultingPublishedReplyCount
 positionOffset
 mergedAt
```

The transport does not hydrate the target topic after the command. Returning the
receipt keeps exact replay deterministic and avoids adding locale, visibility or
profile reads to a manager command.

## Authorization and tenant scope

The resolver first requires the `forum` module to be enabled for the routed
tenant. It then requires an authenticated `AuthContext` containing
`forum_topics:manage`.

Tenant authority comes from `TenantContext`. The optional `tenantId` argument is
an assertion only: omission uses the routed tenant and a different value fails
with the existing GraphQL `PERMISSION_DENIED` contract before the owner service
is called.

The resolver converts the authenticated permission snapshot into
`SecurityContext`. `ForumTopicMergeService` independently rechecks
`forum_topics:manage` and requires a human actor, so a transport wiring defect
cannot bypass owner authorization.

## Idempotency and errors

The first accepted command executes the existing FORUM-21B transaction and
stores one immutable receipt and matching semantic event. An exact retry with
the same operation, source, target, actor and normalized reason returns the same
receipt. Command drift under the same operation ID continues to fail with
`FORUM_TOPIC_MERGE_OPERATION_CONFLICT`.

Forum domain errors flow through `ForumGraphqlErrorExtension`, preserving stable
Forum error codes and retryability. The transport does not reinterpret solution,
category, lifecycle, reply-bound or canonical-resolution conflicts.

Mutation commands intentionally do not follow merged source identities. A stale
source or archived target remains an owner validation failure rather than an
implicit redirect to another mutation target.

## Source-ready coverage

The module-local runtime test uses SQLite owner migrations and the real
`ForumTopicMergeService` through the same adapter function as the resolver. It
covers:

- denial without `forum_topics:manage`;
- tenant override rejection;
- a successful same-category merge;
- exact operation replay returning the same receipt;
- event and operation identity equality.

The integration contract test builds the real `ForumQuery`/`ForumMutation`
schema and verifies the mutation, input and receipt fields are present.

## Compatibility and remaining work

FORUM-21K changes only the additive Forum GraphQL mutation schema. It does not
change merge persistence, events, REST, reads, redirects, GraphQL queries or
existing mutations.

No REST, native Leptos, admin UI, storefront UI or CLI merge command is added.
Competing accepted-solution selection, cross-category merge, split, fork and
reply-range operations remain separate bounded workflows. The canonical
`FORUM-21` entry remains `planned`.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-graphql-transport.mjs
cargo test -p rustok-forum graphql::topic_merge_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_merge_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
