# FORUM-33I unread activity metrics actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / low-cardinality-observation-baseline`

## Rechecked FORUM-33 cursor

The source sequence is now:

- FORUM-33A: bounded topic/category counter reconciliation;
- FORUM-33B: independent counter keyset continuation;
- FORUM-33C: accepted-solution and solution-stat reconciliation;
- FORUM-33D: persisted subscription reconciliation;
- FORUM-33E: persisted mention reconciliation;
- FORUM-33F: read-only Forum -> Search projection convergence status;
- FORUM-33G: Notifications-owned exact-recipient reconciliation dry-run status;
- FORUM-33H: fixed-cardinality locale-resolution observations on `forumUnreadTopics`.

Attachments remain blocked on FORUM-14. Fresh source still has no persisted Forum-owned attachment relation/source-revision authority, so this slice does not infer attachment truth from Media-private state or rich-text references.

## Why unread/activity is the next truthful metric

The canonical FORUM-33 remaining-metrics list includes unread/activity observability. Unlike the still-reserved spam outcomes, Forum already owns and materializes unread truth on the mounted GraphQL path through:

```text
ForumReadModelService::list_topics_with_unread
```

The returned `TopicUnreadReadModel` already contains the exact owner-derived primitives used by the response:

- whether an explicit read state exists;
- unread approved-reply count;
- whether a newer topic revision exists;
- the final `is_unread` projection.

No additional database query, owner call, foreign-module read, write or repair is needed to observe those facts.

## Metric contract

FORUM-33I adds:

```text
rustok_forum_graphql_unread_topic_state_total{state="..."}
```

`state` is fixed to exactly five values with deterministic precedence:

1. `implicit` — no explicit user read-state row exists;
2. `reply_and_revision` — explicit state exists, with unread replies and a newer topic revision;
3. `reply` — explicit state exists with unread replies only;
4. `revision` — explicit state exists with a newer topic revision only;
5. `read` — explicit state exists with neither unread replies nor a newer topic revision.

The classifier treats `unread_count > 0` as reply activity and never exports the numeric unread count as a label.

## Observation semantics, not tenant backlog

This is an observation counter over successful bounded `forumUnreadTopics` results. One returned topic contributes one increment to one state.

It is deliberately not described as:

- the current number of unread topics in a tenant;
- the number of unique topics or users;
- an unread backlog gauge;
- a snapshot-consistent tenant-wide population estimate.

Repeated requests can observe the same topic more than once. A request with `unreadOnly = true` naturally biases observations toward unread states. Those properties are part of the metric contract rather than hidden assumptions.

## Cardinality and privacy boundary

The metric has one bounded label, `state`. It does not use:

- tenant ID;
- user ID;
- topic or category ID;
- locale;
- unread count;
- last-read position or revision;
- title, slug, route or content;
- arbitrary error values.

The existing FORUM-33H locale metric remains unchanged and separate.

## Runtime and failure semantics

The activity observer runs only after the existing bounded owner read succeeds and before the already-existing DTO mapping consumes the page.

Collector registration reuses `rustok_telemetry::register_runtime_collector`. Registration failure remains best-effort telemetry failure: it does not alter the owner read, GraphQL response, cursor, RBAC, visibility, read-state semantics or mutation behavior, and a later observation may retry registration after telemetry initialization.

FORUM-33I adds no migration, queue, worker, job, receipt, retry lane, checkpoint, repair path or database mutation.

## Spam and moderation recheck

Spam-outcome telemetry remains premature. The current FORUM-26D posting-policy evaluator is pure and not mounted as owner enforcement; `DuplicateContent` and `ExternalSpamScore` remain reserved reasons until their owner inputs/contracts exist. FORUM-33 must not manufacture operational spam outcomes from an unexecuted evaluator.

Moderation private application-operation storage also remains outside Forum ownership. A later moderation diagnostic must use an explicit public owner-supported status/read boundary rather than reading `moderation_application_operations` directly.

## Canonical-plan drift

The large canonical Forum implementation plan still lags parts of the merged FORUM-33 execution cursor. This dated packet records the actual source cursor without replacing the entire canonical file through the GitHub contents API and risking unrelated concurrent roadmap loss.

## Next FORUM-33 cursor

Attachments remain blocked on FORUM-14. Spam outcomes remain blocked on real owner enforcement/contracts.

After this unread/activity baseline, recheck broader locale coverage or a permitted Moderation/public-owner diagnostic. Notification/Search lag should only be added if it does not duplicate metrics already owned by those modules.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source check, intentionally not run here:

```bash
node scripts/verify/verify-forum-unread-activity-metrics-source.mjs
```
