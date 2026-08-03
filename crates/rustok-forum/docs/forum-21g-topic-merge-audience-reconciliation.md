# FORUM-21G topic merge audience reconciliation

## Status

`source_ready_maintainer_execution_pending`

FORUM-21G adds one fail-closed follow-up owner command for the source-ready
FORUM-21B same-category topic merge. It reconciles the optional Forum-owned
topic-local audience layer after the source topic becomes the archived, locked
merge tombstone.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-audience-reconciliation.json
```

The owner API is:

```rust
ForumTopicMergeAudienceReconciliationService::reconcile_merge_audience(
    tenant_id,
    merge_operation_id,
    security,
    ReconcileForumTopicMergeAudienceInput {
        operation_id,
        reason,
    },
)
```

The service requires `forum_topics:manage`, a non-nil human actor and a bounded
reason. Source and target identities come only from the immutable FORUM-21B
merge receipt.

## Why arbitrary union is unsafe

Every inherited category audience layer and the optional topic-local layer are
evaluated conjunctively. Inside one `ForumAudienceConstraints` layer, however,
role, trust, Channel, Groups and explicit allow selectors are a union; explicit
deny wins.

Two different topic-local layers therefore cannot be flattened into one existing
row shape while preserving their conjunction. Unioning positive selectors can
broaden visibility, while intersecting selector identifiers can incorrectly
remove viewers that satisfied different selector kinds. FORUM-21G never guesses
at this policy transformation.

## Safe outcome matrix

| Source local layer | Target local layer | Outcome |
| --- | --- | --- |
| absent | absent | `both_unrestricted` |
| absent | present | `target_only_preserved` |
| present | absent | `source_only_moved` |
| present | equal normalized layer | `equal_layers_deduplicated` |
| present | different normalized layer | `FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT` |

`source_only_moved` recreates the complete normalized source layer on the
retained target, including role, trust, Channel, Groups, explicit allow and
explicit deny relations, while preserving the policy row's `updated_at` value.
The source policy and all cascading child rows are then absent.

`equal_layers_deduplicated` preserves the target row and its timestamps and
removes only the equal source layer.

A differing dual-layer conflict creates no audience mutation, semantic event,
receipt or Search invalidation. Both persisted layers remain available for an
explicit future resolution command.

## Bounds

FORUM-21G reuses the canonical Forum audience limits rather than introducing a
second ACL model:

- at most 4 role selectors;
- at most 32 Channel selectors;
- at most 32 Groups selectors;
- at most 100 explicit allow users;
- at most 100 explicit deny users;
- trust level from 0 through 100;
- reconciliation reason at most 500 characters.

Storage that exceeds these limits fails closed before reconciliation.

## Idempotency

`operation_id` is both the command identity and the Forum-local semantic event
identity.

- exact replay resolves the immutable reconciliation receipt before current
  merge or audience state;
- command drift under the same operation ID fails with
  `FORUM_TOPIC_MERGE_AUDIENCE_RECONCILIATION_CONFLICT`;
- one merge receipt may have only one audience reconciliation receipt;
- exact replay validates its semantic event, locks both audience scopes, proves
  source emptiness and returns the stored result without new invalidations.

A dual-layer policy conflict is not recorded as successful reconciliation and
can be retried after an explicit manager resolves the two local policies.

## Lock order and ordinary writes

Ordinary topic audience replacement and reconciliation share the existing
PostgreSQL audience scope identified by tenant and topic with advisory seed 5.

Ordinary set:

```text
category tree lock
→ active non-deleted topic row FOR SHARE
→ topic audience scope
→ exact local-layer replacement
→ topic Search projection invalidation
```

Reconciliation:

```text
tenant reconciliation lock
→ source and target topic rows in sorted UUID order
→ source and target audience scopes in sorted UUID order
→ bounded normalized source and target snapshots
→ safe outcome or fail-closed policy conflict
→ source-emptiness and target-state proof
```

PostgreSQL database triggers acquire the same deterministic audience scope for
policy and child INSERT, UPDATE and DELETE operations. Inserts into any topic
audience table reject archived or soft-deleted topics. SQLite relies on its
writer serialization and applies the same archived/deleted insert guards.

The public `ForumTopicAudiencePolicyService` name points to the transactional
owner facade. Its `set` command now rejects a stale write racing with merge
after the source is archived.

## Merge validation

First execution requires the exact FORUM-21B receipt and independently validates
its `forum.topic.merged` journal record. It then requires:

- the source topic to be the archived and locked merge tombstone;
- the retained target to remain non-archived;
- both current topics to remain non-soft-deleted;
- source, target and category identities to match the merge receipt.

Current topic state alone is never accepted as proof that a merge occurred.

## Atomic state

One successful first-execution transaction performs:

1. tenant-scoped reconciliation serialization;
2. exact replay lookup;
3. one-reconciliation-per-merge lookup;
4. immutable merge receipt and merge-event validation;
5. deterministic topic-row and audience-scope locking;
6. bounded normalized local-layer loading;
7. one safe outcome transformation;
8. source-emptiness and retained-target state proof;
9. one `forum.topic.merge_audience_reconciled` semantic event;
10. one immutable reconciliation receipt;
11. source and target Search projection invalidations;
12. commit.

Any failure rolls back audience changes, semantic event, receipt and projection
invalidations.

## Search projection boundary

Topic-local audience affects storefront eligibility, Search result eligibility,
SEO and notification authorization. A successful first execution therefore
publishes source and retained-target topic invalidations in the same transaction.
Exact replay and policy conflict publish none.

## Persistence guards

`forum_topic_merge_audience_reconciliations` is append-only on PostgreSQL and
SQLite. Tenant-composite foreign keys bind each receipt to:

- the exact merge operation;
- the archived source topic;
- the retained target topic;
- the human actor.

The database requires non-nil identities, different source and target topics,
`event_id = operation_id`, one receipt per merge, a bounded reason and one of the
four safe outcomes. Direct receipt update and deletion fail closed.

## Regression coverage

The source-ready SQLite regression creates real Forum state and covers:

- a complete source-only local layer using role, trust, Channel, Groups, allow
  and deny selectors;
- execution of the real FORUM-21B merge;
- rejection of stale public owner writes and direct archived-topic inserts;
- `source_only_moved` with exact normalized constraints and preserved
  `updated_at`;
- source policy and all child relations becoming empty;
- one semantic event, one immutable receipt and exactly two projection
  invalidations;
- exact replay without new side effects;
- command drift, second reconciliation and receipt mutation failure;
- different dual layers returning
  `FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT` while preserving both policies
  and producing no event, receipt or invalidation;
- missing merge receipt failure.

Maintainer execution remains required.

## Deliberate boundary

This slice does not deliver an explicit manager-selected resolution for two
different local layers. It also does not reconcile accepted-solution policy,
canonical aliases or redirects, notification delivery state, cross-category
merge, split, fork or reply-range workflows.

No REST, GraphQL, native, admin or storefront reconciliation transport is added.
The canonical `FORUM-21` ledger entry remains `planned` until maintainer
execution and the remaining workflow families are delivered.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-audience-reconciliation.mjs
cargo test -p rustok-forum --test topic_merge_audience_reconciliation_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
