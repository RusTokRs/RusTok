# FORUM-21D topic merge read-state reconciliation

## Status

`source_ready_maintainer_execution_pending`

FORUM-21D adds one bounded follow-up owner command for the source-ready
FORUM-21B same-category topic merge. It removes read-state rows from the
archived source topic while preserving every retained-target read-state row
without modification.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-read-state-reconciliation.json
```

The owner API is:

```rust
ForumTopicMergeReadStateReconciliationService::reconcile_merge_read_states(
    tenant_id,
    merge_operation_id,
    security,
    ReconcileForumTopicMergeReadStatesInput {
        operation_id,
        reason,
    },
)
```

The command requires `forum_topics:manage`, a non-nil human actor and a bounded
reason. Source and target identities are derived only from the immutable
FORUM-21B merge receipt.

## Why source high-water is not translated

`forum_topic_read_states` stores one monotonic pair per user and topic:

```text
(last_read_position, last_read_revision)
```

FORUM-21B appends source replies after the target's existing reply positions.
A user may have read source replies while leaving target replies unread. Moving
the source position to the appended target position would necessarily mark all
preceding target replies read. Source topic revision identifiers are also not
target topic revision identifiers.

Therefore this slice intentionally does not calculate a maximum, offset or
union high-water. The conservative policy is:

- target rows are byte-semantic authoritative and remain unchanged;
- a source-only row is deleted without creating a target row;
- when source and target rows both exist, only the source row is deleted;
- after reconciliation the source topic has zero read-state rows.

This may expose previously read source replies as unread after they become part
of the retained target. It never hides unread target content by marking it read.
That trade-off is explicit and fail-safe.

## Bound and idempotency

At most 500 source read-state rows may be reconciled in one operation. Source
rows are ordered by user identity. Target overlap is loaded only for that bounded
user set.

`operation_id` is both the immutable reconciliation command identity and its
Forum-local semantic event identity.

- first execution validates the merge, deletes the bounded source set and stores
  one event and one receipt;
- exact replay resolves the receipt before current merge/topic state, locks the
  source and target read-state scopes, proves source emptiness and returns the
  original result;
- replay does not depend on the source remaining non-soft-deleted;
- command drift under the same operation ID fails with
  `FORUM_TOPIC_MERGE_READ_STATE_RECONCILIATION_CONFLICT`;
- a second operation ID for the same merge receipt also fails closed.

## Lock order and ordinary writes

Single-topic read write:

```text
active non-deleted topic row FOR SHARE
→ per-topic read-state scope
→ public reply/topic revision high-water checks
→ monotonic read-state upsert
```

Raw and visibility-scoped bulk read writes:

```text
exclude deleted and archived candidates
→ active topic rows in sorted UUID order
→ read-state scopes in sorted UUID order
→ bounded high-water calculation
→ monotonic upserts
```

Read-state reconciliation:

```text
tenant reconciliation lock
→ source and target topic rows in sorted UUID order
→ source and target read-state scopes in sorted UUID order
→ bounded source rows ordered by user ID
→ bounded target-overlap lookup
→ source deletion
→ source-emptiness proof
→ event and receipt
```

PostgreSQL installs a `BEFORE INSERT OR UPDATE OR DELETE` read-state trigger that
acquires the same advisory scope used by owner services. Inserts and updates
then require an existing non-archived topic. SQLite service writes use durable
per-topic lock rows and SQLite writer serialization; SQLite triggers reject
identity changes and writes to archived or missing topics.

The existing monotonic database guards remain intact.

## Source merge validation

First execution requires the exact FORUM-21B receipt and independently validates
its `forum.topic.merged` journal record. It then requires:

- source topic remains the archived, locked merge tombstone;
- retained target remains non-archived;
- source, target and category identities match the merge receipt;
- both topic rows remain non-soft-deleted for first execution.

Exact replay is intentionally based on the immutable reconciliation receipt and
event plus source-emptiness proof rather than current topic lifecycle state.

## Atomic transaction

One first-execution transaction performs:

1. tenant reconciliation serialization;
2. exact receipt replay lookup;
3. one-reconciliation-per-merge lookup;
4. source merge receipt and merge-event validation;
5. deterministic topic-row and read-state-scope locking;
6. bounded source read-state load;
7. target overlap lookup for the bounded source user set;
8. classification as source-only or target-overlap;
9. deletion of every source row with exact affected-row validation;
10. source-emptiness proof;
11. one `forum.topic.merge_read_states_reconciled` journal record;
12. one immutable reconciliation receipt;
13. commit.

Any error rolls back source deletion, event and receipt state. No target
read-state insert, update or delete occurs in this transaction.

## Persistence guards

`forum_topic_merge_read_state_reconciliations` is append-only on PostgreSQL and
SQLite. Tenant-composite foreign keys bind the receipt to:

- the exact FORUM-21B merge operation;
- the archived source topic;
- the retained target topic;
- the human actor.

The database requires non-nil identities, different source and target topics,
`event_id = operation_id`, bounded reason/counts, one reconciliation per merge
and exact classification conservation:

```text
source_read_state_count
  = discarded_source_only_count
  + discarded_target_overlap_count
```

Read-state primary topic identity cannot be rewritten. Reconciliation receipt
updates and deletes are rejected.

## Regression coverage

The source-ready SQLite regression creates real Forum topics, replies and read
state, executes the real FORUM-21B merge, and verifies:

- service and direct database writes to the archived source fail;
- three source rows classify as `1 source-only + 2 target-overlap`;
- all source rows are deleted;
- target positions, revisions and timestamps remain exactly unchanged;
- one matching aggregate event and immutable receipt are stored;
- exact replay is side-effect free;
- raw bulk mark-all excludes the archived source and processes only the target;
- command drift and a second operation for the same merge fail with the typed
  conflict;
- direct receipt update and deletion are rejected;
- a missing merge receipt fails before mutation.

The visibility-scoped bulk source is covered statically for the same archived and
lock markers. Maintainer execution remains required.

## Deliberate boundary

This slice does not reconcile:

- topic subscriptions;
- topic tags;
- topic votes;
- topic-level audience relations;
- notification inbox or delivery state;
- source/target accepted-solution policy;
- canonical aliases or redirects;
- cross-category merge, split, fork or reply-range workflows.

No REST, GraphQL, admin or storefront reconciliation transport is added. The
canonical `FORUM-21` ledger entry remains `planned`.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-read-state-reconciliation.mjs
cargo test -p rustok-forum --test topic_merge_read_state_reconciliation_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
