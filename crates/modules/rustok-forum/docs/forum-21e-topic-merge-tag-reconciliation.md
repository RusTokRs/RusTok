# FORUM-21E topic merge tag reconciliation

## Status

`source_ready_maintainer_execution_pending`

FORUM-21E adds one bounded follow-up owner command for the source-ready
FORUM-21B same-category topic merge. It reconciles Forum-owned topic-to-taxonomy
relations after the source topic becomes the archived, locked merge tombstone.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-topic-merge-tag-reconciliation.json
```

The owner API is:

```rust
ForumTopicMergeTagReconciliationService::reconcile_merge_tags(
    tenant_id,
    merge_operation_id,
    security,
    ReconcileForumTopicMergeTagsInput {
        operation_id,
        reason,
    },
)
```

The service requires `forum_topics:manage`, a non-nil human actor and a bounded
reason. Source and target identities come only from the immutable FORUM-21B
merge receipt.

## Set-union policy

`forum_topic_tags` is a Forum-owned relation from one topic to one tenant-owned
Taxonomy term. Membership is a set by `(tenant_id, topic_id, term_id)`.
Reconciliation therefore uses term identity rather than localized tag text:

- a source-only term row moves to the retained target with the same row `id`,
  `term_id`, `tenant_id` and `created_at`;
- when source and target already contain the same term, the target row remains
  authoritative and only the source duplicate is deleted;
- target-only rows are unchanged;
- no Taxonomy term is created, renamed, merged or deleted;
- the archived source has zero tag rows after reconciliation.

Preserving the target row for duplicates keeps its existing relation identity
and creation time. Moving source-only rows avoids manufacturing replacement
relation identities for surviving tag membership.

## Bounds and idempotency

Ordinary topic create/update tag replacement accepts at most 100 normalized,
non-empty distinct tags. The reconciliation owner accepts at most 500 existing
source rows so bounded cleanup remains possible for older or directly imported
data.

`operation_id` is both the command identity and the Forum-local semantic event
identity.

- exact replay resolves the immutable reconciliation receipt before current
  merge or tag state;
- command drift under the same operation ID fails with
  `FORUM_TOPIC_MERGE_TAG_RECONCILIATION_CONFLICT`;
- one merge receipt may have only one tag reconciliation receipt;
- exact replay locks both tag scopes, proves source emptiness and returns the
  stored result without new invalidations or mutations.

## Lock order and ordinary writes

Ordinary topic-tag replacement and reconciliation share one ordering contract.

Ordinary create/update:

```text
active non-deleted topic row FOR SHARE
→ per-topic tag scope
→ bounded taxonomy-term resolution
→ replace relation set
```

Reconciliation:

```text
tenant reconciliation lock
→ source and target topic rows in sorted UUID order
→ source and target tag scopes in sorted UUID order
→ bounded source rows ordered by term ID and row ID
→ target overlap lookup
→ source-only moves and duplicate deletes
→ source-emptiness proof
```

PostgreSQL installs a `BEFORE INSERT OR UPDATE OR DELETE` trigger on
`forum_topic_tags` that acquires the same advisory scope for the old and new
topic identities in deterministic order. Inserts and updates targeting an
archived topic are rejected. SQLite service paths use durable lock rows and
SQLite writer serialization; SQLite triggers also reject inserts and updates
targeting archived topics.

The active-topic check inside `TopicService` occurs after its transaction owns
the topic row. An update that began from stale pre-merge state therefore rolls
back instead of deleting or replacing tags on the archived source.

## Merge validation

First execution requires the exact FORUM-21B receipt and independently validates
its `forum.topic.merged` journal record. It then requires:

- the source topic to be the archived and locked merge tombstone;
- the retained target to remain non-archived;
- source, target and category identities to match the merge receipt;
- both current topic rows to remain non-soft-deleted.

Current topic state alone is never accepted as proof that a merge occurred.

## Atomic state and projection repair

One transaction performs:

1. tenant-scoped reconciliation serialization;
2. exact replay lookup;
3. one-reconciliation-per-merge lookup;
4. immutable merge receipt and merge-event validation;
5. deterministic topic-row and tag-scope locking;
6. bounded source-row and overlap loading;
7. source-only row moves and duplicate deletion;
8. source-emptiness and count-conservation proof;
9. one `forum.topic.merge_tags_reconciled` Forum-local semantic event;
10. one immutable reconciliation receipt;
11. one source-topic and one target-topic projection invalidation;
12. commit.

Tags are Forum Search facets. The source and target invalidations are therefore
part of the same owner transaction as the tag relation changes. A Search
consumer that already processed the original topic-merge invalidation receives
a later exact owner revision for the reconciled tag state rather than relying on
polling or cache expiry.

Any failure rolls back relation changes, semantic event, receipt and both
projection invalidations.

## Persistence guards

`forum_topic_merge_tag_reconciliations` is append-only on PostgreSQL and SQLite.
Tenant-composite foreign keys bind the receipt to:

- the exact merge operation;
- the archived source topic;
- the retained target topic;
- the human actor.

The database requires non-nil identities, different source and target topics,
`event_id = operation_id`, bounded reason/counts, one receipt per merge and exact
count conservation:

```text
source_tag_count
  = moved_source_only_count
  + deduplicated_existing_count
```

Direct receipt update and deletion fail closed.

## Regression coverage

The source-ready SQLite regression creates real Taxonomy and Forum state, then:

- creates source tags `rust`, `shared`, and `source-only`;
- creates target tags `shared`, `target-only`, and `rust`;
- records source row identities and creation times before merge;
- executes the real FORUM-21B merge;
- verifies ordinary `TopicService` tag replacement and direct archived-topic
  insert/update are rejected;
- reconciles `3 = 1 moved source-only + 2 existing duplicates`;
- proves the moved source-only row preserves its `id`, term identity and
  `created_at`;
- proves both duplicate target rows remain byte-semantic authoritative;
- proves the target-only row is unchanged and the source is empty;
- verifies one semantic event, one immutable receipt and source/target projection
  invalidations;
- verifies exact replay is side-effect free;
- verifies command drift, a second reconciliation, missing merge receipt and
  receipt mutation fail closed.

Maintainer execution remains required.

## Deliberate boundary

This slice does not reconcile:

- topic votes;
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
node scripts/verify/verify-forum-topic-merge-tag-reconciliation.mjs
cargo test -p rustok-forum --test topic_merge_tag_reconciliation_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
