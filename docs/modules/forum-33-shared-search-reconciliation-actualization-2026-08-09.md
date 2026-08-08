# FORUM-33F shared Search reconciliation actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / repair-owned-by-search`

## Rechecked FORUM-33 baseline

The FORUM-33 source sequence has been rechecked through:

- FORUM-33A: bounded topic/category counter reconciliation;
- FORUM-33B: independent counter keyset continuation;
- FORUM-33C: accepted-solution eligibility and solution-author statistic reconciliation;
- FORUM-33D: bounded persisted subscription reconciliation;
- FORUM-33E: bounded persisted mention reconciliation.

The canonical Forum implementation plan still describes subscriptions and mentions as future FORUM-33 work even though #3351 and #3352 are already merged. This packet records the current execution cursor without claiming that stale ledger text is implementation truth.

## Attachment cursor is blocked, not complete

The next canonical cursor was attachments, but FORUM-14 remains `planned`. Recheck found no Forum attachment relation table, entity, owner service or stable persisted source-revision contract to reconcile.

FORUM-33 must not manufacture attachment reconciliation over Media-private asset state or infer Forum relation truth from rich-text/media references. Media owns asset lifecycle; Forum may reconcile attachments only after FORUM-14 introduces the explicit Forum-owned typed relation, usage/order/caption and source-revision boundary promised by the canonical ownership model.

Therefore attachments remain **blocked on FORUM-14** and are not marked source-ready by this slice.

## Permitted shared-owner cursor: Forum -> Search

The next implementable FORUM-33 cursor is the already established Forum -> Search projection contract.

Forum owns an append-only projection revision ledger and exposes only bounded neutral facts through `ForumProjectionOwnerRevisionSourcePort`. The server composes that port from the public `ForumEventService`; Search does not read `forum_projection_revision_ledger` or any other Forum-private table.

Search owns its durable convergence state:

- `search_projection_owner_checkpoints`;
- `search_projection_owner_scan_cursors`;
- `search_projection_inbox`;
- the existing `ForumOwnerCheckpointReconciler` repair/rebuild workflow.

This slice does not duplicate or invoke that repair path. It adds only a trusted read-only status surface over the same ownership boundary.

## Bounded convergence diagnosis

`forumSearchProjectionReconciliationStatus` compares the current tenant's Search-owned checkpoint/inbox state with two bounded Forum owner observations through the existing neutral port:

1. if Search has checkpoint revision `N > 0`, request `after_owner_revision = N - 1, limit = 1` to verify the exact Forum revision/event identity stored by Search;
2. request `after_owner_revision = N, limit = 1` to determine whether at least one later Forum owner revision is waiting beyond the Search checkpoint.

No owner-head scan, offset pagination or unbounded revision traversal is added.

The report can surface:

- `checkpoint_behind`: a next Forum owner revision exists after the Search checkpoint;
- `checkpoint_ahead`: Search has a positive checkpoint but Forum no longer exposes that exact revision and exposes no next revision;
- `checkpoint_event_mismatch`: the Search checkpoint event UUID differs from the exact Forum owner revision event UUID;
- `non_terminal_inbox_work`: Search still has Forum-source `pending`, `processing` or `retryable_error` inbox work.

Revision values are exposed through GraphQL as decimal strings so the owner `i64` clock is not truncated to GraphQL `Int` width.

## Admission and composition

The query is mounted only in server builds with `mod-forum` and also checks both runtime module states:

```text
search
forum
```

Tenant identity comes only from `TenantContext`; an authenticated tenant mismatch is rejected.

The operator must have all three effective permissions:

```text
settings:read
forum_categories:manage
forum_topics:manage
```

The existing host-composed `SharedForumProjectionOwnerRevisionSourcePort` is read from `ModuleRuntimeExtensions`; missing composition fails closed instead of falling back to Forum persistence.

## Snapshot semantics

The Search-owned checkpoint and inbox observations are read under one PostgreSQL `REPEATABLE READ READ ONLY` transaction. The existing checkpoint persistence is PostgreSQL-only, so the status surface also fails closed on other backends rather than inventing a weaker duplicate store.

Forum owner revision calls occur through an independent public owner boundary after the local Search snapshot. Consequently this report is a **diagnostic convergence observation**, not a cross-owner serializable repair fence. Concurrent Forum publication or Search reconciliation may legitimately change the next observation.

## Repair ownership remains unchanged

This slice performs no rebuild, checkpoint advance, inbox mutation, retry, cursor mutation or Forum write. The already existing Search-owned `ForumOwnerCheckpointReconciler` remains the only durable projection recovery path.

No direct Forum private-table read, new cross-module port method, migration, dependency change, lockfile update, worker, CLI adapter or new metric family is introduced. Platform Search entrypoint/span/error telemetry is reused.

## Next FORUM-33 cursor

Attachments remain blocked until FORUM-14 lands a real Forum-owned attachment relation boundary.

The next source work is therefore the remaining **permitted shared-owner projection diagnostics and non-duplicative operational metrics**. Each shared-owner slice must use a public owner contract and preserve the authoritative owner's existing repair semantics rather than copying owner state into Forum.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source check:

```bash
node scripts/verify/verify-forum-search-projection-reconciliation-status-source.mjs
```
