---
id: doc://docs/guides/module-metrics.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Module Metrics

This document captures the current baseline set of Prometheus metrics for RusToK
modules. The source of truth for names and labels remains
`crates/rustok-telemetry/src/metrics.rs`; this guide describes how to use
them and what constitutes the minimal operational baseline.

## Baseline Set

### Module Entry Points

```
rustok_module_entrypoint_calls_total{module,entry_point,path}
```

- `module` — module slug, e.g. `rbac` or `comments`
- `entry_point` — public operation name
- `entry_point` — integration path: `library`, `core_runtime`, `bypass`

This metric is needed to see which contract the runtime actually uses and
whether a legacy bypass has returned.

### Module Errors

```
rustok_module_errors_total{module,error_type,severity}
```

- `error_type` — short stable error class (`database`, `validation`,
  `forbidden`, `not_found`)
- `severity` — operational severity (`warning`, `error`)

Use only low-cardinality values. Do not pass request id,
tenant slug, user id, or raw error message.

### Operation Duration and Errors

```
rustok_span_duration_seconds{operation}
rustok_spans_with_errors_total{operation,error_type}
```

- `operation` — stable operation name, e.g. `comments.create_comment`
- `error_type` — same low-cardinality error class

This is the minimum latency/error layer for write-path and library entry-point
operations.

### Read-path Budgets

```
rustok_read_path_requested_limit{surface,path}
rustok_read_path_effective_limit{surface,path}
rustok_read_path_returned_items{surface,path}
rustok_read_path_limit_clamped_total{surface,path}
rustok_read_path_query_duration_seconds{surface,path,query}
rustok_read_path_query_rows{surface,path,query}
```

- `surface` — transport or runtime surface (`rest`, `graphql`, `library`)
- `path` — read-path name
- `query` — stable step within the read-path, e.g. `comments.page`

This set is mandatory for bounded list/read surfaces where `page/per_page`,
SSR feed or batch read path is present.

## What Is Already Instrumented

- `rustok-forum` — public GraphQL/REST read-path for categories, topics and
  replies writes read-path budgets and query metrics.
- `rustok-blog` — public post list/read surfaces writes read-path budgets and
  query metrics.
- `rustok-pages` — public page read-path writes read-path budgets and query
  metrics.
- `rustok-comments` — service entry-points write module entrypoint metrics,
  span duration/error and read-path budget/query metrics for
  `list_comments_for_target`.
- `rustok-content` — orchestration/helper operations write span duration/error,
  and canonical/orchestration runbooks rely on them.

## Minimum Contract for a New Module

If a module adds a new public surface, the minimum baseline is:

1. For each service entry-point, write
   `rustok_module_entrypoint_calls_total`.
2. For errors, write `rustok_module_errors_total` with a short classifier.
3. For write-path or orchestration operations, write
   `rustok_span_duration_seconds` and `rustok_spans_with_errors_total`.
4. For bounded list/read path, write the entire read-path budget/query set.

Without this, the module is considered operationally incomplete.

## Example

```rust
use std::time::Instant;
use rustok_telemetry::metrics;

fn record_entrypoint() {
    metrics::record_module_entrypoint_call("comments", "create_comment", "library");
}

fn finish(operation: &str, started: Instant, result: &Result<(), MyError>) {
    metrics::record_span_duration(operation, started.elapsed().as_secs_f64());
    if let Err(error) = result {
        metrics::record_span_error(operation, error.kind());
        metrics::record_module_error("comments", error.kind(), error.severity());
    }
}
```

## Operator Questions

When troubleshooting a module, first answer three questions:

1. Which integration path is the traffic going through:
   `rate(rustok_module_entrypoint_calls_total{module="comments"}[5m])`
2. Which error-class is growing:
   `sum(rate(rustok_module_errors_total{module="comments"}[5m])) by (error_type,severity)`
3. Where is the read-path losing budget or hitting latency:
   `histogram_quantile(0.95, rate(rustok_read_path_query_duration_seconds_bucket{path="comments.list_comments_for_target"}[5m]))`

## Rules

1. Do not introduce high-cardinality labels.
2. Do not create new metrics if the existing baseline already covers the task.
3. Do not leave a new public read-path without `read_path_*`.
4. Do not leave a new write/orchestration path without `span_*`.
5. Module documentation must list which of its surfaces are already
   instrumented and what is still missing.
