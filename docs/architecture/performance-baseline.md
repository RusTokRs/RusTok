---
id: doc://docs/architecture/performance-baseline.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Performance Baseline

This document captures the repeatable evidence workflow for performance changes in
RusToK.

## Purpose

Before a query rewrite, new index, read-model change or partitioning,
a repeatable baseline must be collected to compare the effect of changes.

A basic performance baseline does not replace optimization, but provides an evidence bundle for
architectural decisions.

## What to Collect

The minimum baseline includes:

- top SQL statements from `pg_stat_statements`
- `EXPLAIN` for hot paths
- tenant-scoped snapshot that can be compared over time

## Where the Implementation Lives

Current task implementation:

- [db_baseline.rs](../../apps/server/src/tasks/db_baseline.rs)

For the search hot path, a live PostgreSQL gate is additionally used:
`crates/rustok-search/tests/postgres_query_plan.rs`. It creates 100,000
temporary tenant-scoped documents, captures `EXPLAIN (ANALYZE, BUFFERS)` and
checks GIN FTS/trigram indexes. Baseline from 2026-06-27: FTS `6.627 ms`,
typo fallback `327.516 ms` on local PostgreSQL 16.

## When to Use

This workflow is needed if any of the following changes:

- a heavy query path
- an index strategy
- a read-side projection
- a caching decision
- storage layout affecting latency

## Recommended Sequence

1. Warm up the target path with representative traffic.
2. Run the baseline task for the desired tenant.
3. Save a JSON artifact for the current date.
4. Apply the query/index/read-model change.
5. Repeat the baseline and compare plans and top statements.

## Limitations

- evidence is only useful if `pg_stat_statements` is enabled on PostgreSQL
- the baseline task itself does not make architectural decisions
- a read-only evidence workflow must not change domain state

## What Not To Do

- do not optimize a query path without a baseline if it affects a common hot path
- do not compare incompatible tenant snapshots
- do not consider a read-model rewrite successful without a repeated baseline

## Related Documents

- [Platform Data Schema](./database.md)
- [Domain Event Flow Contract](./event-flow-contract.md)
- [Platform Architecture Overview](./overview.md)
