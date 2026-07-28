# FORUM-26I approved-post index hardening

Status: source-ready / unvalidated

## Delivered boundary

This slice closes the pre-enforcement performance debt recorded by FORUM-26H
for the authoritative `ApprovedPosts` posting-policy fact.

The owner query and its semantics remain unchanged. The migration adds bounded
partial author indexes for the exact predicates already used by
`ForumApprovedPostsFactPort`, and dedicated PostgreSQL and SQLite source proofs
bind those index shapes to the aggregate query.

## Index contract

`forum_topics` receives
`idx_forum_topics_tenant_author_retained (tenant_id, author_id)` with the
partial predicate:

```sql
author_id IS NOT NULL AND deleted_at IS NULL
```

This index serves the exact tenant/user count of retained topics. Topic
lifecycle status remains intentionally absent because `open`, `closed`, and
`archived` are lifecycle states, not moderation approval states.

`forum_replies` receives
`idx_forum_replies_tenant_author_approved_retained
(tenant_id, author_id, topic_id)` with the partial predicate:

```sql
author_id IS NOT NULL AND status = 'approved' AND deleted_at IS NULL
```

The trailing `topic_id` supports the required tenant-scoped parent-topic join.
The parent topic lookup and `topic.deleted_at IS NULL` predicate remain part of
the owner query because a retained reply beneath a soft-deleted topic must not
contribute.

Both indexes are partial rather than full-table compatibility indexes. Pending,
rejected, hidden, flagged, deleted-status, soft-deleted, and anonymous-author
rows do not occupy the synchronous approved-post lookup indexes.

## Runtime proof contract

`tests/approved_posts_index_sqlite.rs` applies the module migrations, executes
`EXPLAIN QUERY PLAN` over the exact SQLite aggregate shape, and requires both
partial indexes to appear. It also inspects `sqlite_master` to retain the exact
column and predicate contract.

`tests/approved_posts_index_postgres.rs` applies the module migrations in an
isolated schema, disables sequential scans for capability proof, executes
`EXPLAIN (FORMAT JSON)` over the exact PostgreSQL aggregate shape, and requires
both index names in the plan. It also inspects `pg_indexes` for the retained
column and predicate contract.

The PostgreSQL module-test bootstrap now creates the minimal platform-owned
`users (id, tenant_id)` identity fixture before Forum migrations. FORUM-26A
introduced a tenant-composite user reference for trust state, so the previous
Forum-only bootstrap could no longer apply the complete migration chain in an
isolated schema. The fixture exposes only the identity columns consumed by
Forum and does not copy platform authentication or profile state.

These are source-ready executable proofs. No runtime output is claimed until a
maintainer runs the commands below.

## Migration and compatibility

PostgreSQL and SQLite use the same index names, ordered columns, and logical
partial predicates. The migration is additive and requires no backfill or
owner-state rewrite. Existing rows become index entries automatically when
they satisfy the predicates.

Rollback drops only the two new indexes. It does not modify topics, replies,
trust state, policy facts, or transport contracts. Removing the indexes returns
the owner query to its FORUM-26H performance profile without changing its
result.

## Explicit exclusions

This slice adds no:

- posting-policy evaluation or precedence change;
- topic, reply, edit, or bump owner enforcement;
- policy configuration persistence or administration;
- new owner fact or fact-value semantics;
- distributed rate-limit reservation, commit, release, or counters;
- duplicate-content hashing or retained fingerprint;
- external or AI spam-scoring call;
- trust-state write or automatic promotion/demotion;
- event, worker, GraphQL, REST, OpenAPI, admin, or storefront surface.

## Remaining FORUM-26 scope

The authoritative active-flag and moderation-history adapters remain the next
fact-owner boundary. Reputation, usage windows, bump age, policy persistence,
owner enforcement, shared rate-limit execution, duplicate fingerprints,
optional scoring, administration, transports, UI, and maintainer runtime
evidence also remain open.

Missing capabilities must continue to produce explicit unavailable facts. No
owner may infer moderation, reputation, or rate-limit state from
`forum_user_stats`, reply-status totals, local counters, or policy heuristics.

## Validation status

The following commands were not run by the implementation agent:

```text
cargo test -p rustok-forum --test approved_posts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test approved_posts_index_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-forum-approved-posts-index-hardening.mjs
node scripts/verify/verify-forum-approved-posts-posting-facts.mjs
node scripts/verify/verify-forum-approved-posts-index-debt.mjs
cargo xtask module validate forum
```
