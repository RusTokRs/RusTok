# FORUM-33D subscription reconciliation actualization — 2026-08-08

Status: `source-ready / maintainer-execution-open / repair-open`

## Rechecked baseline

The merged FORUM-33 sequence is source-complete through:

- FORUM-33A: bounded topic/category counter reconciliation;
- FORUM-33B: independent UUID keyset continuation for the counter shapes;
- FORUM-33C: accepted-solution eligibility and `forum_user_stats.solution_count` reconciliation.

The next explicit source cursor recorded by FORUM-33C was subscriptions. Existing FORUM-11 subscription ownership and FORUM-21C topic-merge subscription reconciliation were re-read before selecting this slice.

## Owner boundary

`ForumSubscriptionReconciliationService` is a read-only diagnostic owner under `services::subscription::reconciliation`.

It reconciles persisted Forum-owned rows only. It deliberately does **not** infer that a topic author or reply participant must have a subscription merely because an auto-subscribe policy exists: participation policy controls future owner commands and is not a retroactive projection.

The service also does not read Profiles-owned user lifecycle/privacy facts or Notifications-owned delivery state. Those remain shared-owner or downstream concerns.

## Bounded shapes and cursors

Topic and category subscriptions are independently bounded to the existing FORUM-33 default 100 / hard 500 page size.

Neither subscription table has a single row UUID: each row is identified by tenant plus `(target_id, user_id)`. Continuation therefore uses strict composite keysets rather than OFFSET:

```text
topic:    (topic_id, user_id) > (topicAfter, topicUserAfter)
category: (category_id, user_id) > (categoryAfter, categoryUserAfter)
```

Both components of a supplied cursor are required together. An exhausted shape preserves its previous cursor while the other shape may continue.

GraphQL exposes the current-tenant report as:

```text
forumSubscriptionReconciliationReport(
  limit: Int,
  topicAfter: UUID,
  topicUserAfter: UUID,
  categoryAfter: UUID,
  categoryUserAfter: UUID
)
```

The response returns independent composite topic/category cursors, `hasMore*` flags, inspected-row counts, page-local `clean`, and drift rows.

## Drift classes

The report detects only invariants already owned by Forum:

- `target_missing`: a persisted subscription row no longer resolves to its same-tenant Forum target;
- `merged_topic_source_subscription`: a topic-subscription row remains attached to an immutable topic-merge source identity; actual move/dedup repair remains owned by the existing FORUM-21C reconciliation command;
- `muted_preferences_invalid`: a `muted` row violates the existing schema/service rule requiring all notification flags off and digest disabled;
- `revision_invalid`: a persisted optimistic revision is not positive.

A reversible ordinary `archived` topic is intentionally not classified as drift merely because it is archived. FORUM-21 merge-source detection uses immutable merge history instead of assuming all archived topics are permanent tombstones.

## Admission, snapshot, and observability

GraphQL derives tenant identity only from `TenantContext`, rejects auth/tenant mismatch, and requires both effective permissions:

```text
forum_categories:manage
forum_topics:manage
```

The owner service independently reauthorizes the same two Manage scopes through the canonical Forum RBAC helper before database work.

Each report page runs in one database snapshot:

- PostgreSQL: `REPEATABLE READ READ ONLY`;
- SQLite: one transaction snapshot.

The service reuses platform module-entrypoint/span/error telemetry and adds no duplicate Forum metric family.

## Deliberately open

This slice adds no repair mutation, row deletion, subscription move, deduplication, policy backfill, user/profile inference, delivery repair, schema migration, dependency change, or CLI adapter.

Any generic write repair still requires the FORUM-33 operator boundary: explicit repair RBAC, dry-run semantics, durable audit, idempotent job/receipt state, bounded retry/recovery, and retained PostgreSQL/SQLite execution evidence.

The next source reconciliation cursor is mentions, followed by attachments and shared-owner projections where authoritative contracts permit it.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation, or `git diff --check` was executed while preparing this slice.

Suggested source check:

```bash
node scripts/verify/verify-forum-subscription-reconciliation-source.mjs
```
