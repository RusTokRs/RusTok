# Documentation `rustok-index`

`rustok-index` is the platform-owned cross-module relational Index Engine. It
addresses the same problem class as the Medusa Index Module: source modules
publish generic schemas, records, mutations, and links; Index materializes them
into optimized relational storage and serves structured cross-module queries
without runtime fan-out.

## Purpose

- publish canonical schema, mutation, query, source, and rebuild contracts;
- keep ingestion, storage, query planning, rebuild, and consistency semantics in
  the module;
- provide server, storefront, admin, and `rustok-search` with a stable substrate
  for cross-module filtering, projection, sorting, count, and pagination;
- scale reads and rebuilds independently from source-module query paths.

## Scope

- schema and link registry;
- generic records and mutations;
- incremental ingestion and inbox deduplication;
- PostgreSQL storage and distributed coordination;
- link-aware validation, planning, and SQL compilation;
- cursor pagination, exact count, and bounded offset compatibility;
- bootstrap, rebuild, checkpointing, reconciliation, and drift repair;
- operator health, lag, failure, and rebuild controls.

## Excluded scope

- text relevance and ranking;
- typo tolerance, synonyms, autocomplete, and search UX;
- external search-engine connectors;
- source-module table reads from Index core;
- source-specific Product, Content, or Flex logic in the generic engine.

## Rewrite policy

Backward compatibility with the rejected source-specific implementation is not
a goal. Conflicting code, migrations, ports, adapters, tests, evidence, and
documentation are deleted rather than preserved through compatibility layers.

The old v1 read/rebuild ports, fallback adapters, FBA registry/evidence,
catch-all document model, empty query services, and search-specific FTS helper
have been removed. Source-specific indexers and migrations are the next M0
deletion target.

## Status

- Rewrite: `in_progress`
- Current milestone: `M0/M1 - hard reset and domain core`
- FFA: `in_progress`
- FBA: `in_progress`
- Active public core: `rustok_index::domain`

## Verification

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`
- PostgreSQL integration, property, planner snapshot, rebuild recovery, and
  benchmark suites introduced by later milestones.

## Related documents

- [Crate README](../README.md)
- [Live implementation plan](./implementation-plan.md)
- [Index Engine rewrite ADR](../../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
