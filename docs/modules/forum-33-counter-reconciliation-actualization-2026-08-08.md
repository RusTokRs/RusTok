# FORUM-33 counter reconciliation actualization — 2026-08-08

Status: `in-progress / bounded-owner-report-and-cursors-source-ready / repair-and-runtime-evidence-open`

## Rechecked cursor

FORUM-32 no longer has an unimplemented Forum/Page Builder source path. Its owner preview, property editing, browser harness, direct runtime-authorization harness and deployed server-function attestation are source-ready; maintainer execution, the Pages reference-consumer gate and observed Wave remain outside this source slice.

FORUM-33A established the first read-only owner counter reconciliation page. FORUM-33B closes the next source gap by adding independent bounded continuation for the topic and category shapes without adding repair authority.

## Delivered FORUM-33A/B source

`ForumCounterReconciliationService` is owned by `rustok-forum` and is exposed through the Forum GraphQL operator query:

```text
forumCounterReconciliationReport(
  limit: Int,
  topicAfter: UUID,
  categoryAfter: UUID
)
```

All arguments are optional. Omitting both cursors preserves the original first-page behavior. The existing owner `report(...)` method remains as a compatibility first-page wrapper, while `report_page(...)` is the bounded continuation entrypoint.

The query is available only when the Forum module is enabled for the request tenant and the authenticated principal has both effective permissions:

```text
forum_categories:manage
forum_topics:manage
```

Tenant identity is taken only from the trusted `TenantContext`. The query does not accept another tenant id and rejects an auth/tenant context mismatch.

Authorization is deliberately enforced twice. GraphQL rejects missing manage permissions early for a clear transport error, then builds the exact `SecurityContext` from the authenticated permission snapshot. `ForumCounterReconciliationService` independently requires `ForumCategories/Manage` and `ForumTopics/Manage` through the existing Forum `services::rbac::enforce_scope` helper before opening a database snapshot. A future CLI or host adapter therefore cannot gain reconciliation access merely by bypassing the GraphQL resolver.

## Counter invariants

The owner report checks the existing publication-accounting invariants without mutating them:

1. `forum_topics.reply_count` equals the number of `approved` Forum replies for that topic;
2. `forum_categories.topic_count` equals the number of current Forum topic rows in that category;
3. `forum_categories.reply_count` equals the number of `approved` Forum replies across topics in that category.

These are the same public-accounting semantics used by topic creation/deletion, reply publication transitions and reply removal. Pending, rejected, hidden, flagged and deleted reply rows do not contribute to public reply counters.

## Independent bounded continuation

Topic and category continuation are independent because the two shapes may have very different cardinalities. The owner query accepts:

- `topic_after` / GraphQL `topicAfter`;
- `category_after` / GraphQL `categoryAfter`.

Each SQL shape remains ordered by its owner UUID and uses a strict keyset predicate (`id > cursor`) rather than OFFSET pagination. Each query still fetches at most `effective_limit + 1` rows. The response returns:

- `has_more_topics` and `topic_cursor`;
- `has_more_categories` and `category_cursor`.

The returned cursor is the last inspected owner id for that shape. If a shape returns no new rows, its output cursor preserves the supplied input cursor instead of resetting to `None`. An operator can therefore keep echoing an exhausted category cursor while advancing topics, or vice versa, without rescanning the exhausted shape from the beginning.

This makes tenants larger than the hard 500-row page cap traversable without unbounded work. It deliberately does not keep a database transaction open across HTTP/GraphQL requests. Snapshot consistency is page-local: writes that occur behind an already-returned cursor are observed by the next full reconciliation scan rather than by a long-lived cross-request snapshot. The report remains diagnostic/read-only, so no repair decision may treat a multi-page scan as a serializable write fence.

## Bounded snapshot database shape

Every page executes exactly two tenant-scoped aggregate queries inside one database snapshot:

- one grouped topic/reply query;
- one grouped category/topic/reply query.

PostgreSQL uses one `REPEATABLE READ READ ONLY` transaction so concurrent Forum writes cannot make the topic and category observations within a page come from different database snapshots. SQLite uses one transaction for both reads; the first read establishes the consistent SQLite snapshot. Failed report construction explicitly rolls the transaction back, while a completed report commits the read-only snapshot.

The default limit is 100 and the hard maximum is 500. The service avoids per-subject N+1 queries and never scans another tenant through this API. PostgreSQL and SQLite have explicit initial-page and keyset-continuation statements. Other backends fail closed rather than approximating SQL or snapshot semantics.

## Observability

The owner service reuses platform telemetry instead of defining redundant Forum-only metric families:

- `rustok_module_entrypoint_calls_total` records the `counter_reconciliation_report` library entrypoint;
- `rustok_span_duration_seconds` records bounded reconciliation latency;
- `rustok_spans_with_errors_total` and `rustok_module_errors_total` record failed owner report execution.

The report itself is the bounded operational source for exact counter-drift details. A later FORUM-33 metrics slice may add only metrics that cannot be represented truthfully by the existing platform baseline.

## Deliberately not added

This slice does **not** add a repair mutation or CLI repair command. The canonical FORUM-33 repair boundary still requires all of the following before any owner counter can be changed:

- explicit operator RBAC;
- dry-run semantics;
- durable audit records;
- idempotent job/receipt state;
- bounded retry/recovery behavior;
- PostgreSQL/SQLite execution evidence.

A separate `rustok-forum-cli` adapter remains deferred until its workspace dependency and `Cargo.lock` update are synchronized with maintainer-run dependency tooling.

## Remaining FORUM-33 scope

- retain SQLite and PostgreSQL execution evidence for clean/drifted first pages, multi-page cursor traversal, exhausted-one-side continuation and page-local snapshot behavior;
- add bounded reconciliation for accepted-solution state, subscriptions, mentions, attachments and shared-owner projections where authoritative owner contracts permit it;
- add the audited/idempotent repair job boundary before any write repair;
- decide the platform CLI adapter together with its synchronized dependency/lock update;
- add only non-duplicative Forum operational metrics for moderation age/approval/report lag, notification/search lag, unread/activity signals, locale fallback, spam outcomes and remaining owner drift;
- compose operator UI/CLI surfaces over the same owner report rather than duplicating SQL or policy.

No Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI or runtime evidence was executed while preparing this source slice.
