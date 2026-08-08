# FORUM-33 reconciliation actualization — 2026-08-08

Status: `in-progress / bounded-counter-and-solution-reconciliation-source-ready / repair-and-runtime-evidence-open`

## Rechecked cursor

FORUM-32 no longer has an unimplemented Forum/Page Builder source path. Its owner preview, property editing, browser harness, direct runtime-authorization harness and deployed server-function attestation are source-ready; maintainer execution, the Pages reference-consumer gate and observed Wave remain outside this source slice.

FORUM-33A established the first read-only counter reconciliation page. FORUM-33B added independent bounded topic/category continuation. FORUM-33C closes the next source gap by adding accepted-solution authority and solution-author-stat reconciliation without adding repair authority.

## Delivered counter reconciliation source

`ForumCounterReconciliationService` remains the read-only owner for publication-accounting drift and is exposed through:

```text
forumCounterReconciliationReport(
  limit: Int,
  topicAfter: UUID,
  categoryAfter: UUID
)
```

It checks:

1. `forum_topics.reply_count` against `approved` replies;
2. `forum_categories.topic_count` against current topic rows;
3. `forum_categories.reply_count` against `approved` replies across category topics.

Topic/category traversal remains independent, bounded to default 100 / hard 500 rows per shape, keyset-based with strict `id > cursor`, and page-local snapshot consistent. `clean` is page-local; whole-tenant clean requires exhausting both cursor chains with every page clean.

## Delivered accepted-solution reconciliation source

`ForumSolutionReconciliationService` is a sibling Forum owner service exposed through the same GraphQL operator query object:

```text
forumSolutionReconciliationReport(
  limit: Int,
  solutionAfter: UUID,
  solutionStatAfter: UUID
)
```

The two shapes are independent because the number of accepted solutions and the number of Forum user-stat rows may differ substantially.

### Accepted-solution authority

`forum_solutions` remains the authoritative accepted-solution relation. Existing production tenant-integrity constraints bind each row to exactly one same-tenant topic and one same-tenant reply in that exact topic, with one solution per `(tenant_id, topic_id)` and a solution reply unique within the tenant.

The reconciliation report adds the runtime-lifecycle invariant that the selected reply must still be `approved`. Pending, rejected, hidden, flagged or deleted/nonexistent replies are not operationally valid accepted solutions. The report therefore emits `accepted_reply_eligibility` drift for an authoritative solution row whose exact topic/reply relation is unavailable or whose reply is no longer approved.

The solution shape is bounded and ordered by `forum_solutions.topic_id`; continuation uses strict `topic_id > solutionAfter`, never OFFSET.

### Solution-author statistic projection

`forum_user_stats.solution_count` is a Forum-owned projection, not solution authority. Its expected value is the number of authoritative solution rows whose exact reply is still `approved` and authored by that user.

The report detects both sides of projection drift:

- `solution_author_stat_missing`: an approved accepted solution has a non-null author but no `forum_user_stats` row for that author;
- `solution_author_stat_count`: an existing user-stat row has a stored `solution_count` different from the number of approved accepted solutions authored by that user, including stale positive counts when no accepted solutions remain.

The stat shape first bounds the `forum_user_stats` user ids, then aggregates accepted solutions only for that bounded page. It is ordered by user UUID and continues with strict `user_id > solutionStatAfter`; it does not fan out into one query per user.

## Admission and tenant boundary

Both reconciliation fields use the same `ForumReconciliationQuery` admission helper. They are available only when the Forum module is enabled for the request tenant and the authenticated principal has both effective permissions:

```text
forum_categories:manage
forum_topics:manage
```

Tenant identity is taken only from trusted `TenantContext`. Caller input cannot select another tenant, and auth/tenant mismatch is rejected.

Authorization is deliberately enforced twice. GraphQL rejects missing manage permissions early, then constructs an exact `SecurityContext` permission snapshot. Both owner reconciliation services independently require `ForumCategories/Manage` and `ForumTopics/Manage` through the canonical Forum `services::rbac::enforce_scope` helper before database work begins. A later CLI/host adapter therefore cannot bypass owner admission by avoiding GraphQL.

## Snapshot and bounded-work semantics

Every counter page and every solution page uses one database snapshot for all shapes in that report call:

- PostgreSQL: `REPEATABLE READ READ ONLY`;
- SQLite: one transaction whose first read establishes the page snapshot.

Each independent shape fetches at most `effective_limit + 1` rows. Returned cursors preserve the supplied cursor when a shape returns no new rows, allowing one exhausted side to remain parked while another advances.

Snapshot consistency is deliberately page-local. A multi-page diagnostic scan does not retain a transaction across HTTP/GraphQL requests. Rows created or changed behind an already-returned cursor are observed on the next full scan. Neither `clean=true` on one page nor a multi-page scan is a serializable repair fence.

## Observability

The services reuse platform telemetry instead of introducing duplicate Forum-only metric families:

- module entrypoint calls for `counter_reconciliation_report` and `solution_reconciliation_report`;
- platform span duration;
- platform span/module error counters.

These source-ready instrumentation calls do not claim observed production metrics or provider SLO health.

## Deliberately not added

This slice does **not** add a repair mutation or CLI repair command. The canonical FORUM-33 repair boundary still requires all of the following before any owner counter/relation projection can be changed:

- explicit operator RBAC;
- dry-run semantics;
- durable audit records;
- idempotent job/receipt state;
- bounded retry/recovery behavior;
- PostgreSQL/SQLite execution evidence.

A separate `rustok-forum-cli` adapter remains deferred until its workspace dependency and `Cargo.lock` update are synchronized with maintainer-run dependency tooling.

## Remaining FORUM-33 scope

- retain SQLite and PostgreSQL execution evidence for counter clean/drift pages, multi-page traversal, exhausted-one-side continuation and page-local snapshot behavior;
- retain SQLite and PostgreSQL execution evidence for accepted solution eligibility, missing solution-author stat, stale/mismatched `solution_count`, multi-page solution/stat cursors and concurrent page-local snapshot behavior;
- add bounded reconciliation for subscriptions next, then mentions, attachments and shared-owner projections where authoritative owner contracts permit it;
- add the audited/idempotent repair job boundary before any write repair;
- decide the platform CLI adapter together with its synchronized dependency/lock update;
- add only non-duplicative Forum operational metrics for moderation age/approval/report lag, notification/search lag, unread/activity signals, locale fallback, spam outcomes and remaining owner drift;
- compose operator UI/CLI surfaces over the same owner services rather than duplicating SQL or policy.

No Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI or runtime evidence was executed while preparing this source slice.
