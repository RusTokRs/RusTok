# Documentation `rustok-index`

`rustok-index` is the platform-owned cross-module relational Index Engine. It
addresses the same problem class as the Medusa Index Module: source modules
publish generic schemas, records, mutations, and links; Index materializes them
into optimized relational storage and serves structured cross-module queries
without runtime fan-out.

## Purpose

- publish canonical schema, mutation, query, source, and rebuild contracts;
- keep ingestion, storage, planning, rebuild, and consistency semantics inside
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
- rebuild, checkpointing, reconciliation, and drift repair;
- operator health, lag, failure, and rebuild controls.

## Excluded scope

- text relevance and ranking;
- typo tolerance, synonyms, autocomplete, and search UX;
- external search-engine connectors;
- source-module table reads from Index core;
- source-specific Product, Content, or Flex logic in the engine.

## Rewrite policy

Backward compatibility with the rejected implementation is not a goal.
Conflicting code is deleted instead of preserved through compatibility layers.

M0 removed the complete old implementation: v1 ports and evidence, catch-all
documents, query placeholders, Content/Product/Flex indexers and models, direct
source SQL, all legacy migrations, admin table reads, source-domain dependencies,
old runtime configuration/scheduler/errors, and server dispatcher composition.

## Status

- Rewrite: `in_progress`
- Current milestone: `M1 - domain core and schema registry`
- FFA: `in_progress`
- FBA: `in_progress`
- M0 code reset: `complete`
- Active public core: `rustok_index::domain`

## Verification

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- `npm run verify:index:fba`
- `npm run verify:index:runtime-fallback-smoke`

## Related documents

- [Crate README](../README.md)
- [Live implementation plan](./implementation-plan.md)
- [Index Engine rewrite ADR](../../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
