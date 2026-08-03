# FORUM-21F topic merge vote reconciliation

## Status

`source_ready_maintainer_execution_pending`

FORUM-21F adds one bounded follow-up owner command for the source-ready
FORUM-21B same-category topic merge. It reconciles Forum-owned topic votes after
the source topic becomes the archived, locked merge tombstone.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-vote-reconciliation.json
```

The owner API is:

```rust
ForumTopicMergeVoteReconciliationService::reconcile_merge_votes(
    tenant_id,
    merge_operation_id,
    security,
    ReconcileForumTopicMergeVotesInput {
        operation_id,
        reason,
    },
)
```

The service requires `forum_topics:manage`, a non-nil human actor and a bounded
reason. Source and target identities come only from the immutable FORUM-21B
merge receipt.

## Target-authority policy

`forum_topic_votes` is keyed by `(tenant_id, topic_id, user_id)`. One user may
therefore have a vote on both the source and retained target before merge.
Reconciliation uses voter identity rather than score arithmetic:

- a source-only vote moves to the retained target with the same `user_id`,
  `value`, `created_at` and `updated_at`;
- when source and target votes have the same value, the target row remains
  authoritative and the source duplicate is deleted;
- when source and target values differ, the target row and value remain
  authoritative and the source conflict is deleted;
- target-only rows are unchanged;
- the archived source has zero topic-vote rows after reconciliation.

The command never adds two votes from the same user together. A retained target
vote records the user's current authority for that retained discussion.

Reply votes are not remapped. FORUM-21B moves replies without changing reply
identities, so `forum_reply_votes.reply_id` remains valid and already follows the
moved reply.

## Bounds and idempotency

The reconciliation owner accepts at most 10,000 existing source rows. The bound
keeps one owner transaction finite while supporting substantially larger vote
sets than relation-oriented reconciliation slices.

`operation_id` is both the command identity and the Forum-local semantic event
identity.

- exact replay resolves the immutable reconciliation receipt before current
  merge or vote state;
- command drift under the same operation ID fails with
  `FORUM_TOPIC_MERGE_VOTE_RECONCILIATION_CONFLICT`;
- one merge receipt may have only one vote reconciliation receipt;
- exact replay locks both vote scopes, proves source emptiness and returns the
  stored result without new mutations or events.

## Lock order and ordinary writes

Ordinary topic-vote writes and reconciliation share one ordering contract.

Ordinary set and clear:

```text
active non-deleted topic row FOR SHARE
→ per-topic vote scope
→ upsert or delete one voter row
```

Reconciliation:

```text
tenant reconciliation lock
→ source and target topic rows in sorted UUID order
→ source and target vote scopes in sorted UUID order
→ bounded source rows ordered by user ID
→ target overlap lookup
→ source-only moves and duplicate/conflict deletes
→ source-emptiness proof
```

PostgreSQL installs a `BEFORE INSERT OR UPDATE OR DELETE` trigger on
`forum_topic_votes` that acquires the same advisory scope for old and new topic
identities in deterministic order. Inserts and updates targeting an archived
topic are rejected. SQLite service paths use durable lock rows and SQLite writer
serialization; SQLite triggers also reject inserts and updates targeting
archived topics.

The active-topic check occurs after `VoteService` owns its transaction. A set or
clear operation racing with topic merge either completes before archival or
observes the archived source and rolls back.

## Merge validation

First execution requires the exact FORUM-21B receipt and independently validates
its `forum.topic.merged` journal record. It then requires:

- the source topic to be the archived and locked merge tombstone;
- the retained target to remain non-archived;
- source, target and category identities to match the merge receipt;
- both current topic rows to remain non-soft-deleted.

Current topic state alone is never accepted as proof that a merge occurred.

## Atomic state

One transaction performs:

1. tenant-scoped reconciliation serialization;
2. exact replay lookup;
3. one-reconciliation-per-merge lookup;
4. immutable merge receipt and merge-event validation;
5. deterministic topic-row and vote-scope locking;
6. bounded source-row and overlap loading;
7. source-only row moves and duplicate/conflict deletion;
8. source-emptiness and count-conservation proof;
9. one `forum.topic.merge_votes_reconciled` Forum-local semantic event;
10. one immutable reconciliation receipt;
11. commit.

Any failure rolls back vote changes, semantic event and receipt.

## Search projection boundary

The current Forum Search projection does not contain topic vote score or
current-user vote state. FORUM-21F therefore emits no Search invalidation.
Topic and list response vote summaries are loaded from `forum_topic_votes` by
`VoteService`, so the retained target score reflects the reconciled rows on the
next read.

A future Search contract that projects vote score must add explicit invalidation
ownership rather than relying on this source-ready boundary.

## Persistence guards

`forum_topic_merge_vote_reconciliations` is append-only on PostgreSQL and SQLite.
Tenant-composite foreign keys bind the receipt to:

- the exact merge operation;
- the archived source topic;
- the retained target topic;
- the human actor.

The database requires non-nil identities, different source and target topics,
`event_id = operation_id`, bounded reason/counts, one receipt per merge and exact
count conservation:

```text
source_vote_count
  = moved_source_only_count
  + deduplicated_equal_count
  + target_authority_conflict_count
```

Direct receipt update and deletion fail closed.

## Regression coverage

The source-ready SQLite regression creates real Forum state, then:

- creates one source-only voter, one target-only voter, one equal overlap and
  one conflicting overlap;
- records source and target vote values and timestamps before merge;
- executes the real FORUM-21B merge;
- verifies ordinary `VoteService` set and clear on the source fail;
- verifies direct archived-topic insert and update fail;
- reconciles `3 = 1 moved + 1 equal duplicate + 1 target-authority conflict`;
- proves the moved source-only row preserves value and both timestamps;
- proves all retained target rows remain byte-semantic authoritative;
- proves the source is empty and the retained target score is correct;
- verifies one semantic event and one immutable receipt;
- verifies exact replay is side-effect free;
- verifies command drift, a second reconciliation, missing merge receipt and
  receipt mutation fail closed.

Maintainer execution remains required.

## Deliberate boundary

This slice does not reconcile:

- topic-level audience relations;
- notification inbox or delivery state;
- accepted-solution policy;
- canonical aliases or redirects;
- cross-category merge;
- split, fork or reply-range operations.

No REST, GraphQL, admin or storefront reconciliation transport is added. The
canonical `FORUM-21` ledger entry remains `planned` until maintainer execution
and the remaining workflow families are delivered.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-vote-reconciliation.mjs
cargo test -p rustok-forum --test topic_merge_vote_reconciliation_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
