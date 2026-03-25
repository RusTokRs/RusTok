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
  - `trackSearchClick`
  - `triggerSearchRebuild`
- Prometheus endpoint: `/metrics`

Persistent analytics are stored in `search_query_logs` and `search_query_clicks`.

## State model

`searchDiagnostics.state` uses three operator-facing states:

- `healthy`: indexed documents are in sync or the tenant has no indexable sources yet
- `lagging`: at least one document is stale or max lag exceeds the current threshold
- `bootstrap_pending`: the tenant has indexable source records, but no `search_documents` yet

## Important Prometheus metrics

### Query path

- `rustok_search_queries_total{surface,engine,status}`
- `rustok_search_query_duration_seconds{surface,engine}`
- `rustok_search_results_returned{surface,engine}`
- `rustok_search_zero_results_total{surface,engine}`

`surface` is expected to be one of:

- `search_preview`
- `storefront_search`

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
- low-CTR query leaderboard
- abandonment query leaderboard
- query-intelligence candidates

## Admin diagnostics interpretation

When reviewing `searchDiagnostics`:

- high `stale_documents` means ingestion is falling behind or failing on a subset of entities
- high `max_lag_seconds` means some records were updated much later than they were indexed
- `bootstrap_pending` means a first rebuild is required or ingestion never caught up after module enablement

When reviewing `searchLaggingDocuments`:

- start with the largest `lag_seconds`
- compare `updated_at` and `indexed_at`
- check whether the same entity repeatedly reappears after rebuild

If the same entity keeps returning to the lagging list, inspect source data and
event delivery before re-running more rebuilds.

When reviewing `searchAnalytics`:

- high zero-result rate usually points to missing synonyms, redirects, or weak content coverage
- top queries with low average results are good candidates for relevance tuning
- repeated zero-result queries should feed synonym and dictionary work
- low CTR with high result volume often means the best match is buried and needs boost or pinning
- high abandonment usually means users saw results but did not trust or value the first page

`trackSearchClick` feeds CTR and abandonment by writing to `search_query_clicks`.
The analytics layer waits a short grace window before treating a successful
query as abandoned so fresh queries are not marked as failures immediately.

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
- p95 of `rustok_search_query_duration_seconds` above agreed SLO
- repeated errors in `rustok_search_indexing_operations_total{status!="success"}`
- non-zero `rustok_search_bootstrap_pending_tenants_total` after rollout
- growing `rustok_search_lagging_tenants_total`

## Dashboard starter panels

Recommended first dashboard:

- query volume by `surface`
- p50/p95 search latency by `surface`
- zero-result rate by `surface`
- indexing operations by `operation` and `status`
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
