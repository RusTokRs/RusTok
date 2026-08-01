# FORUM-23B2G2B3D16 topic-move Search proof

## Status

`source_ready_maintainer_execution_pending`

FORUM-23B2G2B3D16 adds the remaining source-ready `LINK-FORUM-03` lifecycle
proof that was blocked on a real Forum owner command: move one existing active
topic from its current category to another active category, retain topic and
reply identity, and converge Search category scope through the existing typed
invalidation, one-inbox, production reconciler and storefront path.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-topic-move-proof.json
```

The executable proof is:

```text
apps/server/tests/forum_versioned_invalidation_topic_move.rs
```

Successful maintainer execution writes:

```text
target/forum-search-link-forum-03-topic-move-evidence.json
```

The artifact is generated only after every assertion succeeds and the isolated
PostgreSQL schema is removed.

## Owner dependency

The proof calls the exported FORUM-21A owner API directly:

```rust
ForumTopicMoveService::move_topic(
    tenant_id,
    topic_id,
    admin_security,
    MoveForumTopicInput {
        operation_id,
        target_category_id,
        reason,
    },
)
```

It does not update `forum_topics.category_id`, category counters, move receipts,
Forum journal rows, outbox rows or Search tables through direct SQL. Direct SQL
is used only for evidence reads and the isolated test user fixture.

The move result must correlate with:

- one immutable `forum_topic_move_operations` receipt;
- one Forum-local `forum.topic.moved` journal row;
- `event_id = operation_id` in both owner records;
- the same topic, source category, target category, actor, bounded reason and
  published-reply count;
- exactly three new projection revisions in owner order: topic, source category,
  target category.

## PostgreSQL profile

The test creates a unique PostgreSQL schema and applies the real migrations for:

1. `rustok-outbox`;
2. `rustok-taxonomy`;
3. `rustok-forum`;
4. `rustok-search`.

It uses `RUSTOK_SEARCH_TEST_DATABASE_URL`, then
`RUSTOK_FORUM_TEST_DATABASE_URL`, then `DATABASE_URL`. A non-PostgreSQL or absent
URL skips the runtime profile and cannot create an evidence artifact.

External Iggy is not used in this bounded proof. D3-D5 and D10 retain the broker
profiles; D16 focuses on the owner command, typed contract ingress, durable
Search inbox, production projection source, reconciler and storefront category
scope.

## Baseline phase

The fixture creates two public categories, one public topic in the source
category and one approved reply. These real owner commands must produce exactly
four contiguous projection revisions:

1. source category create: `forum` scope;
2. target category create: `forum` scope;
3. topic create: source `forum_category` scope;
4. approved reply create: source `forum_category` scope.

For every revision the proof resolves the real legacy root envelope and the
distinct caused typed envelope from `sys_events`. The typed envelope enters
Search only through `ForumSearchContractIngress`.

After `ForumProjectionReconciler` catches up, the source category storefront
filter must return the exact topic and reply, while the target category filter
must return neither item nor visible facet buckets. Search documents must expose
source category identifiers in both `facets.category_id` and
`payload.category_id`; source category counters are `1/1`, target counters are
`0/0`.

## Move phase

The test invokes one real `ForumTopicMoveService` command with a fresh operation
ID. It verifies the immutable receipt and semantic journal identity before
accepting the projection trace.

The move must append exactly these owner revisions:

5. `forum_topic:<topic_id>`;
6. `forum_category:<source_category_id>`;
7. `forum_category:<target_category_id>`.

Every revision must retain one root envelope, one distinct caused typed
envelope and one Search inbox row. Search-owned `ingest_sequence` must increase
within each delivery phase, but it is never compared numerically with the
Forum-owned revision clock.

The topic invalidation rebuilds the Forum tenant projection, so the approved
reply document must move with the topic rather than retaining a stale source
category facet. Source and target category invalidations then refresh their
counter documents.

After reconciliation:

- the exact topic ID remains unchanged;
- the exact reply ID remains unchanged;
- both documents expose the target category in facets and payload;
- source category counters are `0/0`;
- target category counters are `1/1`;
- source category storefront searches return zero items and zero visible facet
  buckets;
- target category storefront searches return the exact topic and reply through
  production execution and Forum owner reauthorization.

## Exact replay phase

The proof calls the owner command again with the same operation ID, topic,
target, actor and normalized reason. Replay must return the original
`ForumTopicMoveResult` and create none of the following:

- owner revision 8;
- another root invalidation;
- another typed envelope;
- another Search inbox row;
- another move receipt;
- another `forum.topic.moved` journal row;
- additional reconciler work.

The final owner ledger must be exactly contiguous revisions 1 through 7, and its
root event identities must equal the retained root event set.

## Evidence shape

The generated artifact records one passed scenario:

```text
topic_move_category_scope
```

Its facts include:

- tenant, source category, target category, topic and reply identities;
- immutable move owner receipt and semantic event payload;
- all seven owner revision rows;
- baseline and move root/typed/inbox correlations;
- baseline and moved Search documents;
- explicit retained-identity, old-scope exclusion, new-scope inclusion and
  replay-idempotency booleans.

`source_commit` is read from `git rev-parse HEAD`; hand-edited or stale evidence
is not accepted as current source proof.

## Deliberate boundaries

This slice does not:

- run the FORUM-21A SQLite owner regression or provide its PostgreSQL concurrency
  promotion evidence;
- add REST, GraphQL, native, admin or storefront topic-move mutations;
- add canonical URL aliases, redirects or tombstones;
- implement merge, split, fork or reply-range operations;
- use external Iggy;
- mutate the reviewed D0, D12, D13, D14 or D15 evidence contracts;
- mark `FORUM-21`, `FORUM-23` or `LINK-FORUM-03` complete.

A later reviewed assembler may consume D13-D16 only after the required runtime
artifacts are generated on the same exact source commit and independently
reviewed.

## Maintainer verification

```bash
node scripts/verify/verify-forum-search-link-forum-03-topic-move-proof.mjs
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-server \
  --test forum_versioned_invalidation_topic_move \
  -- --nocapture --test-threads=1
```

No command above was run by the implementation agent, per maintainer request.
