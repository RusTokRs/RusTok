# rustok-search observability runbook

## Purpose

This runbook explains how operators should observe, diagnose, and recover
`rustok-search`.

It covers:

- admin diagnostics surfaces
- admin analytics surfaces
- Prometheus metrics exposed by the module
- common failure modes
- rebuild and recovery procedures

## Operator entry points

Use these entry points first:

- Leptos admin: `Search -> Overview`, `Search -> Diagnostics`
- Next admin: `/dashboard/search`
- GraphQL:
  - `searchDiagnostics`
  - `searchAnalytics`
  - `searchLaggingDocuments`
  - `searchConsistencyIssues`
  - `trackSearchClick`
  - `triggerSearchRebuild`
- Prometheus endpoint: `/metrics`

Persistent analytics are stored in `search_query_logs` and `search_query_clicks`.

## State model

`searchDiagnostics.state` uses four operator-facing states:

- `healthy`: indexed documents are in sync or the tenant has no indexable sources yet
- `inconsistent`: projection drift exists because source rows are missing in `search_documents` or search rows are orphaned
- `lagging`: at least one document is stale or max lag exceeds the current threshold
- `bootstrap_pending`: the tenant has indexable source records, but no `search_documents` yet

## Important Prometheus metrics

### Query path

- `rustok_search_queries_total{surface,engine,status}`
- `rustok_search_query_duration_seconds{surface,engine}`
- `rustok_search_results_returned{surface,engine}`
- `rustok_search_zero_results_total{surface,engine}`
- `rustok_search_slow_queries_total{surface,engine}`
- `rustok_search_rate_limit_outcomes_total{surface,namespace,outcome}`

`surface` is expected to be one of:

- `search_preview`
- `storefront_search`
- `storefront_search_suggestions`

### Indexing and rebuild path

- `rustok_search_indexing_operations_total{operation,entity,status}`
- `rustok_search_indexing_duration_seconds{operation,entity}`
- `rustok_module_errors_total{module="search",error_type,severity}`

Common `operation` values:

- `rebuild_tenant`
- `rebuild_content_scope`
- `rebuild_product_scope`
- `upsert_node`
- `upsert_node_locale`
- `reindex_category`
- `upsert_product`

### Fleet-level search gauges from `/metrics`

- `rustok_search_metrics_collection_status`
- `rustok_search_documents_total`
- `rustok_search_public_documents_total`
- `rustok_search_stale_documents_total`
- `rustok_search_tenants_with_documents_total`
- `rustok_search_lagging_tenants_total`
- `rustok_search_bootstrap_pending_tenants_total`
- `rustok_search_max_lag_seconds`

### Persistent analytics from GraphQL/admin

`searchAnalytics` currently exposes:

- rolling query volume
- zero-result rate
- click-through rate
- abandonment rate
- average query latency
- average results per query
- distinct query count
- top queries
- zero-result query leaderboard
- slow-query leaderboard
- low-CTR query leaderboard
- abandonment query leaderboard
- query-intelligence candidates

### Admin audit trail signals

- `rustok_search_audit_events_total{action,status}`

Expected `action` values:

- `update_settings`
- `trigger_rebuild`

Expected `status` values:

- `published`
- `publish_failed`

## Admin diagnostics interpretation

When reviewing `searchDiagnostics`:

- high `stale_documents` means ingestion is falling behind or failing on a subset of entities
- non-zero `missing_documents` means source rows exist but search projection rows are absent
- non-zero `orphaned_documents` means `search_documents` still contains rows whose source entity or locale is gone
- high `max_lag_seconds` means some records were updated much later than they were indexed
- `bootstrap_pending` means a first rebuild is required or ingestion never caught up after module enablement

When reviewing `searchLaggingDocuments`:

- start with the largest `lag_seconds`
- compare `updated_at` and `indexed_at`
- check whether the same entity repeatedly reappears after rebuild

If the same entity keeps returning to the lagging list, inspect source data and
event delivery before re-running more rebuilds.

When reviewing `searchConsistencyIssues`:

- `missing` issues point to projection gaps and usually justify scoped or tenant-wide rebuilds
- `orphaned` issues point to stale search rows that were not deleted after source removal or locale drift
- if counts stay non-zero after rebuild, inspect ingestion handlers and source event coverage before retrying again

When reviewing `searchAnalytics`:

- high zero-result rate usually points to missing synonyms, redirects, or weak content coverage
- high slow-query rate usually points to broad result sets, missing operator
  filters, or ranking/query shapes that need indexing review
- top queries with low average results are good candidates for relevance tuning
- repeated zero-result queries should feed synonym and dictionary work
- low CTR with high result volume often means the best match is buried and needs boost or pinning
- high abandonment usually means users saw results but did not trust or value the first page

`trackSearchClick` feeds CTR and abandonment by writing to `search_query_clicks`.
The analytics layer waits a short grace window before treating a successful
query as abandoned so fresh queries are not marked as failures immediately.

When reviewing rate-limit behavior:

- rising `rustok_search_rate_limit_outcomes_total{outcome="exceeded"}` on
  `storefront_search` usually means the public surface is receiving bursts from
  a small set of clients or bots
- non-zero `rustok_search_rate_limit_outcomes_total{outcome="backend_unavailable"}`
  means the shared limiter backend is degraded and storefront search protection
  has partially failed open at the infrastructure layer

When reviewing audit delivery:

- `rustok_search_audit_events_total{status="publish_failed"}` should normally
  stay at zero
- if publish failures rise while admin mutations still succeed, inspect the
  outbox/event transport path rather than the search control plane itself

## Recovery procedures

### Tenant-wide rebuild

Use when:

- a tenant is `bootstrap_pending`
- lag affects many entities across content and product domains
- search schema or projector logic changed

Action:

1. Trigger `triggerSearchRebuild(targetType: "search")`.
2. Watch `rustok_search_indexing_operations_total{operation="rebuild_tenant"}`.
3. Confirm `searchDiagnostics.state` returns to `healthy`.

### Content-only rebuild

Use when lag or data drift is limited to content entities.

Action:

1. Trigger `triggerSearchRebuild(targetType: "content")`.
2. Re-check `searchLaggingDocuments`.
3. Confirm content-focused stale rows disappear.

### Product-only rebuild

Use when lag or data drift is limited to products.

Action:

1. Trigger `triggerSearchRebuild(targetType: "product")`.
2. Re-check `searchLaggingDocuments`.
3. Confirm product-focused stale rows disappear.

### Entity-scoped rebuild

Use when a single node or product is stale or malformed.

Action:

1. Trigger `triggerSearchRebuild` with `targetType` and `targetId`.
2. Re-run diagnostics for the tenant.
3. If the document remains stale, inspect source rows and event flow.

## Alert ideas

Recommended starter alerts:

- sustained increase in `rustok_search_zero_results_total`
- sustained increase in `rustok_search_slow_queries_total`
- p95 of `rustok_search_query_duration_seconds` above agreed SLO
- repeated errors in `rustok_search_indexing_operations_total{status!="success"}`
- non-zero `rustok_search_bootstrap_pending_tenants_total` after rollout
- growing `rustok_search_lagging_tenants_total`
- sustained `rustok_search_rate_limit_outcomes_total{outcome="backend_unavailable"} > 0`
- sustained increase in `rustok_search_audit_events_total{status="publish_failed"}`

## Dashboard starter panels

Recommended first dashboard:

- query volume by `surface`
- p50/p95 search latency by `surface`
- zero-result rate by `surface`
- slow-query count by `surface`
- rate-limit outcomes by `surface` and `outcome`
- indexing operations by `operation` and `status`
- audit-event publication by `action` and `status`
- lagging tenants
- bootstrap-pending tenants
- max lag seconds

## Safety notes

- Search query paths are read-only and must not trigger rebuilds.
- Rebuilds are transactional inside `rustok-search`.
- Search ingestion runs with dispatcher retries, but repeated failures still
  require operator review.

## When to escalate

Escalate beyond routine rebuilds when:

- tenant-wide rebuild succeeds but lag immediately returns
- `rustok_module_errors_total{module="search"}` keeps increasing
- query latency increases without corresponding indexing pressure
- bootstrap-pending tenants stay non-zero after initial rollout
