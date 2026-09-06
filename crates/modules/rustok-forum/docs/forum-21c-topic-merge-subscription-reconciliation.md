# FORUM-21C topic merge subscription reconciliation

## Status

`source_ready_maintainer_execution_pending`

FORUM-21C adds one bounded follow-up owner command for the source-ready
FORUM-21B same-category topic merge. It reconciles topic subscriptions after a
real immutable merge receipt exists and the source topic has become the archived,
locked redirect-ready tombstone.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-subscription-reconciliation.json
```

The owner API is:

```rust
ForumTopicMergeSubscriptionReconciliationService::reconcile_merge_subscriptions(
    tenant_id,
    merge_operation_id,
    security,
    ReconcileForumTopicMergeSubscriptionsInput {
        operation_id,
        reason,
    },
)
```

The command requires `forum_topics:manage`, a non-nil human actor and a bounded
reason. Source and target identities are never supplied by the caller; they are
derived from the immutable FORUM-21B merge receipt.

## Target-authority policy

The retained target subscription is always authoritative. The reconciliation
never combines notification preferences and never upgrades delivery implicitly.
For each source subscriber:

- when no target row exists, the source row moves to the target with the same
  level, notification flags, digest mode, `last_notified_at` and `created_at`;
  the owner advances `revision` exactly once and refreshes `updated_at` because
  changing the subscription target is an owner mutation;
- when both rows have equal delivery state, the source row is deleted and counted
  as an equal deduplication;
- when both rows differ, the source row is deleted and counted as a target-authority
  conflict;
- target-only rows are untouched.

The equality classification compares effective persisted delivery state, not
revision or audit timestamps. The target revision remains authoritative even when
source and target delivery state is equal.

## Bound and idempotency

At most 500 source topic subscription rows may be reconciled in one operation.
The owner sorts source rows by user identity and processes every source row in one
transaction.

`operation_id` is the immutable reconciliation command and semantic event
identity. The exact receipt is resolved before current merge or subscription
state. Reusing the operation ID with a different merge receipt, actor or
normalized reason fails with:

```text
FORUM_TOPIC_MERGE_SUBSCRIPTION_RECONCILIATION_CONFLICT
```

A merge receipt may have only one reconciliation receipt. A second operation ID
for the same merge receipt also fails closed instead of silently treating an
already-reconciled merge as a new command.

## Lock order and ordinary writes

Normal topic subscription writes and reconciliation share one ordering contract.

Normal service write:

```text
topic row FOR SHARE
→ per-topic subscription scope
→ current subscription row/revision
→ insert or revision-CAS update
```

Reconciliation:

```text
tenant reconciliation lock
→ source and target topic rows in sorted UUID order
→ source and target subscription scopes in sorted UUID order
→ bounded source rows ordered by user ID
→ matching target rows
→ move/delete classification
```

PostgreSQL also installs a `BEFORE INSERT OR UPDATE OR DELETE` table trigger that
acquires the same sorted per-topic advisory scopes. This covers existing
auto-subscribe triggers and direct database mutations, not only service calls.
SQLite writes one durable lock row per tenant/topic in service paths and relies on
SQLite writer serialization for all database-triggered mutation. Topic
subscription inserts and updates targeting archived topics are rejected by both
backends.

Therefore:

- a write committed before FORUM-21B archive is visible to reconciliation;
- a service write racing the topic merge holds a shared topic lock and completes
  before the merge acquires its exclusive topic lock;
- a write after source archival fails at the service and database boundaries;
- an auto-subscribe or direct PostgreSQL target insert cannot race source-only
  movement into the same target user key.

## Source merge validation

The owner requires the exact FORUM-21B receipt and independently validates its
`forum.topic.merged` semantic journal record. It then requires:

- the source topic to remain archived and locked;
- the target topic to remain non-archived;
- both topic identities and the category to match the merge receipt;
- source and target rows to remain non-deleted.

The reconciliation does not infer merge history from current topic state alone.

## Atomic transaction and events

One owner transaction performs:

1. tenant reconciliation serialization;
2. exact replay lookup;
3. one-reconciliation-per-merge lookup;
4. merge receipt and merge-event validation;
5. deterministic topic and subscription locking;
6. bounded source-row load;
7. target-row lookup for only the bounded source user set;
8. source-only primary-key movement with one revision advance or source-row
   deletion under target authority;
9. proof that no source subscriptions remain;
10. one `forum.topic.merge_subscriptions_reconciled` journal record;
11. one immutable reconciliation receipt;
12. commit.

The existing subscription-table triggers continue to emit
`forum.subscription.changed.v1` for row movement and deletion in the same owner
transaction. The new Forum-local reconciliation event records the aggregate
operation and classified counts; it does not replace the existing per-row event
contract or change the shared `rustok-events` catalog.

Any error rolls back all row movement, deletion, subscription-change events,
reconciliation event and receipt state.

## Persistence guards

`forum_topic_merge_subscription_reconciliations` is append-only on PostgreSQL
and SQLite. Tenant-composite foreign keys bind each receipt to:

- the exact merge operation;
- the archived source topic;
- the retained target topic;
- the human actor.

The database requires non-nil identities, different source and target topics,
`event_id = operation_id`, bounded reason/counts, one reconciliation per merge,
and exact count conservation:

```text
source = moved_source_only + deduplicated_equal + target_authority_conflict
```

Direct topic-subscription INSERT/UPDATE targeting an archived topic is rejected.
Direct reconciliation receipt UPDATE/DELETE is also rejected.

## Regression coverage

The source-ready SQLite regression creates real Forum topics and subscription
rows, executes the real FORUM-21B merge, and then verifies:

- service and direct database source writes are rejected after archival;
- the automatic source/target topic-author subscriptions form one equal pair and
  preserve the retained target author row at revision 1;
- a source-only row preserves delivery state and advances revision from 7 to 8;
- a second explicit equal pair preserves the target revision;
- a conflicting pair preserves the muted target without preference escalation;
- the classified conservation is `4 = 1 moved + 2 equal + 1 conflict`;
- all source rows disappear and the target user union contains five rows;
- one matching semantic event and immutable receipt are stored;
- exact replay is side-effect free;
- command drift and a second operation for the same merge fail with the typed
  conflict;
- direct receipt update and deletion are rejected;
- a missing merge receipt fails before any state mutation.

## Deliberate boundary

This slice changes only topic subscriptions. It does not reconcile:

- topic read state;
- topic tags;
- topic votes;
- topic-level audience relations;
- category subscriptions;
- notification inbox/fan-out/delivery state;
- source and target accepted-solution policy;
- canonical aliases or redirects;
- cross-category merge, split, fork or reply-range workflows.

No public REST, GraphQL, admin or storefront transport is added.

The canonical `FORUM-21` ledger entry remains `planned`. FORUM-21C may advance
only after maintainer execution on one exact checkout.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-subscription-reconciliation.mjs
cargo test -p rustok-forum --test topic_merge_subscription_reconciliation_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
