---
id: doc://crates/rustok-forum/docs/forum-16-postgres-runtime-proof.md
kind: implementation_record
language: en
status: source_ready
owners:
  - rustok-forum
last_reviewed: 2026-07-24
canonical_plan: doc://crates/rustok-forum/docs/implementation-plan.md
---

# FORUM-16 PostgreSQL read-state runtime proof

This slice publishes executable PostgreSQL coverage for the FORUM-16 owner
contracts. It does not claim successful runtime evidence until the maintainer
runs the test against PostgreSQL and records the resulting output.

## Concurrent-device monotonicity

`topic_read_state_postgres` creates two independent PostgreSQL connections for
the same tenant, user and topic. One connection advances the approved-reply
position while the other advances the immutable topic revision from a lower
position. Both owner commands must succeed and the durable read row must converge
to the component-wise maximum rather than whichever transaction commits last.

The scenario also attempts a direct SQL regression after convergence. The
PostgreSQL monotonic trigger must reject reducing either high-water mark.

## Production-sized unread aggregate

The isolated fixture creates 128 topics, 8,192 approved replies and 512 topic
revisions. A bounded 100-topic request contains four deterministic classes:

- 32 topics with one unread approved reply;
- 32 topics with an unread topic revision only;
- 32 explicitly read topics;
- 4 unseen topics without a read-state row.

The test calls the public `ForumReadModelService::summarize_topic_ids` owner
contract and checks every class. Hidden and rejected replies are not seeded as
public rows and therefore cannot contribute to the approved-reply aggregate.

## Query-plan evidence

The test runs the proof-only SQL mirror of the canonical owner aggregate through
`EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, FORMAT JSON)`. The natural plan must return
exactly the bounded 100-topic page, contain no per-topic `SubPlan`, and keep
`Actual Loops` within the topic-page bound.

A second plan disables sequential scans only to prove index capability. It must
show index access for the topic read-state primary key, a tenant/topic reply
position index and the tenant/topic revision index. This is not a claim that the
production planner must always prefer an index over a sequential scan, and this
record deliberately publishes no latency threshold.

The static verifier binds the proof query's material joins and predicates to the
owner source so the EXPLAIN scenario cannot silently drift into a different
unread policy.

## Verification

```bash
export RUSTOK_FORUM_TEST_DATABASE_URL=postgres://...
cargo test -p rustok-forum --test topic_read_state_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-forum-read-state-runtime-proof.mjs
```

Tests, Cargo, verifiers and CI were not run while publishing this source-ready
slice. FORUM-16 remains `in_progress` until the maintainer records successful
PostgreSQL execution and the visibility-scoped storefront category/all-read
commands can be implemented after the shared FORUM-20 visibility policy.
