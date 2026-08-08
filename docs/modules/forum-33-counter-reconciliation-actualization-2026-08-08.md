# FORUM-33 counter reconciliation actualization — 2026-08-08

Status: `in-progress / bounded-owner-report-source-ready / repair-and-runtime-evidence-open`

## Rechecked cursor

FORUM-32 no longer has an unimplemented Forum/Page Builder source path. Its owner preview, property editing, browser harness, direct runtime-authorization harness and deployed server-function attestation are source-ready; maintainer execution, the Pages reference-consumer gate and observed Wave remain outside this source slice.

The next real Forum source gap is FORUM-33: analytics, observability and reconciliation integrated with platform operations.

## Delivered FORUM-33A source

This slice adds a read-only `ForumCounterReconciliationService` owned by `rustok-forum` and exposes it through a dedicated Forum GraphQL operator query:

```text
forumCounterReconciliationReport(limit: Int)
```

The query is available only when the Forum module is enabled for the request tenant and the authenticated principal has both effective permissions:

```text
forum_categories:manage
forum_topics:manage
```

Tenant identity is taken only from the trusted `TenantContext`. The query does not accept another tenant id and rejects an auth/tenant context mismatch.

## Counter invariants

The owner report checks the existing publication-accounting invariants without mutating them:

1. `forum_topics.reply_count` equals the number of `approved` Forum replies for that topic;
2. `forum_categories.topic_count` equals the number of current Forum topic rows in that category;
3. `forum_categories.reply_count` equals the number of `approved` Forum replies across topics in that category.

These are the same public-accounting semantics used by topic creation/deletion, reply publication transitions and reply removal. Pending, rejected, hidden, flagged and deleted reply rows do not contribute to public reply counters.

## Bounded database shape

The report executes exactly two tenant-scoped aggregate queries:

- one grouped topic/reply query;
- one grouped category/topic/reply query.

Each query requests at most `effective_limit + 1` rows so the service can return `has_more_topics` / `has_more_categories` without an unbounded count. The default limit is 100 and the hard maximum is 500. The service therefore avoids per-subject N+1 queries and never scans another tenant through this API.

The implementation contains explicit PostgreSQL and SQLite statements because those are the Forum-supported production/test backends. Other backends fail closed rather than approximating SQL semantics.

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

A separate `rustok-forum-cli` adapter was considered but not added here because adding a new selected CLI provider requires a synchronized workspace dependency and `Cargo.lock` update. This task intentionally does not generate or edit the lockfile without maintainer-run dependency tooling.

## Remaining FORUM-33 scope

- retain SQLite and PostgreSQL execution evidence for clean and intentionally drifted counter fixtures;
- add bounded reconciliation for accepted-solution state, subscriptions, mentions, attachments and shared-owner projections where authoritative owner contracts permit it;
- add the audited/idempotent repair job boundary before any write repair;
- decide the platform CLI adapter together with its synchronized dependency/lock update;
- add only non-duplicative Forum operational metrics for moderation age/approval/report lag, notification/search lag, unread/activity signals, locale fallback, spam outcomes and remaining owner drift;
- compose operator UI/CLI surfaces over the same owner report rather than duplicating SQL or policy.

No Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI or runtime evidence was executed while preparing this source slice.
