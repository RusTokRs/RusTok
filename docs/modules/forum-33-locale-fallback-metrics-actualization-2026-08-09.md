# FORUM-33H locale fallback metrics actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / low-cardinality-baseline`

## Rechecked FORUM-33 cursor

The source sequence is now:

- FORUM-33A: bounded topic/category counter reconciliation;
- FORUM-33B: independent counter keyset continuation;
- FORUM-33C: accepted-solution and solution-stat reconciliation;
- FORUM-33D: persisted subscription reconciliation;
- FORUM-33E: persisted mention reconciliation;
- FORUM-33F: read-only Forum -> Search projection convergence status;
- FORUM-33G: Notifications-owned exact-recipient reconciliation dry-run status.

Attachments remain blocked on FORUM-14 because current Forum still has no persisted Forum-owned attachment relation/source-revision authority to reconcile. This slice does not infer attachment truth from Media-private state or rich-text references.

## FORUM-33H scope

The canonical FORUM-33 remaining-metrics list includes locale fallback observability. Forum already resolves localized read models through the shared Content fallback algorithm and exposes both `requested_locale` and `effective_locale`, but the rechecked source had no dedicated Forum locale-resolution metric.

This slice adds a first bounded baseline to the already mounted GraphQL `forumUnreadTopics` read path. It intentionally does not instrument every Forum locale-aware surface in one change.

The existing owner call remains authoritative:

```text
ForumReadModelService::list_topics_with_unread
```

Only after that bounded owner call succeeds, GraphQL records one observation per returned topic from the already materialized owner DTO. There is no additional database query, owner call, fallback pass, retry or mutation.

## Metric contract

The metric is:

```text
rustok_forum_graphql_locale_resolution_total{resource="unread_topic",outcome="..."}
```

`resource` is fixed to `unread_topic` for this baseline. `outcome` has exactly three bounded values:

- `exact`: at least one translation exists and the effective locale equals the requested locale;
- `fallback`: at least one translation exists and the effective locale differs from the requested locale;
- `missing`: the owner DTO reports no available locale at all.

The `fallback` bucket deliberately combines tenant fallback, platform fallback and first-available fallback. The metric answers whether fallback was needed, not which locale was selected.

## Cardinality and privacy boundary

No locale values are metric labels. The metric also carries no tenant ID, user ID, topic ID, category ID, title, slug, route, content or arbitrary error code.

This keeps the metric fixed-cardinality and prevents tenant-controlled locale strings from becoming Prometheus series.

The source uses the platform runtime collector registration boundary. Registration failure is best-effort telemetry failure: it does not change the GraphQL response or Forum owner read result, and a later observation may retry registration after telemetry initialization.

## Read semantics remain unchanged

FORUM-33H does not change:

- `resolve_graphql_locale`;
- tenant fallback selection;
- `resolve_by_locale_with_fallback` ordering;
- Forum RBAC or tenant scoping;
- unread calculation;
- topic visibility/audience policy;
- cursor order or page limits;
- read-state mutation behavior;
- DTO requested/effective/available locale fields.

Only successful `forumUnreadTopics` pages are observed. Owner/read errors remain errors and are not rewritten into locale outcomes.

## Deliberately not claimed

This baseline does not claim complete Forum locale observability. Category tree/list, topic route resolution, reply reads, storefront audience topic/reply paths, Search presentation and admin transports remain unchanged. They should only receive later instrumentation when it is useful and does not duplicate an existing owner metric.

Likewise this slice does not add moderation, notification/search lag, unread-activity or spam-outcome metrics. Those remain separate FORUM-33 candidates with their own ownership checks.

## Canonical-plan drift

The canonical Forum implementation plan still lags the merged FORUM-33D/E/F/G execution cursor. This dated FORUM-33H packet records the actual source cursor without replacing the large canonical document through a whole-file update that could overwrite unrelated concurrent work.

## Next FORUM-33 cursor

Attachments remain blocked on FORUM-14.

After this locale baseline, continue with another non-duplicative operational metric or permitted shared-owner diagnostic. Moderation should only be observed through an owner-supported public status/read contract rather than `moderation_application_operations` private storage. Notification/Search lag should not duplicate metrics already owned by those modules. Broader locale coverage and spam outcomes remain candidates after a fresh source recheck.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source check:

```bash
node scripts/verify/verify-forum-locale-fallback-metrics-source.mjs
```
