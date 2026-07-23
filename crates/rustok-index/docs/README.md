# Documentation `rustok-index`

`rustok-index` is the platform-owned cross-module relational Index Engine. Its
functional target is the same problem class as the Medusa Index Module: source
modules publish schemas, records, and links; Index materializes them into an
optimized relational store and serves structured cross-module queries without
runtime fan-out.

## Purpose

- publish the canonical schema, mutation, query, and rebuild contracts;
- keep ingestion, storage, query planning, rebuild, and consistency semantics
  inside the module;
- provide server, storefront, admin, and `rustok-search` with a stable internal
  substrate for cross-module filtering, projection, sorting, count, and
  pagination;
- scale reads and rebuilds independently from source-module query paths.

## Scope

- schema and link registry;
- generic index records and mutations;
- incremental ingestion and inbox deduplication;
- PostgreSQL index storage and distributed coordination;
- link-aware query validation, planning, and SQL compilation;
- cursor pagination, count, and bounded compatibility offset pagination;
- bootstrap, rebuild, checkpointing, reconciliation, and drift repair;
- operator-facing health, lag, failure, and rebuild controls.

## Excluded scope

- text relevance and ranking;
- typo tolerance, synonyms, autocomplete, and search UX;
- external search-engine connectors;
- direct reads of source-module tables from Index core;
- source-specific Product, Content, or Flex logic in the generic engine.

Those search concerns remain owned by `rustok-search`. Source modules own their
schema declarations and conversion from domain state/events to generic Index
records and mutations.

## Rewrite policy

The existing source-specific CQRS/read-model implementation is legacy. Because
the project is in early development, rejected internal APIs, migrations, tests,
adapters, and projections may be deleted or replaced rather than preserved by
compatibility layers.

Every implementation PR must update the task checkboxes and progress log in the
live implementation plan.

## Status

- Rewrite: `in_progress`
- Current milestone: `M0 - hard reset and architecture lock`
- FFA: `in_progress`
- FBA: `in_progress`
- Legacy FBA contracts remain temporary until the new engine contracts have
  compiled and live provider-consumer evidence.

## Verification

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- `npm run verify:index:fba`
- PostgreSQL integration, property, planner snapshot, rebuild recovery, and
  benchmark suites introduced by the milestones.

## Related documents

- [Crate README](../README.md)
- [Live implementation plan](./implementation-plan.md)
- [Index Engine rewrite ADR](../../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
