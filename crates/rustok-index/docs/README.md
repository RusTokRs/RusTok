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

- versioned schema and link registry;
- generic records and mutations;
- explicit tenant/locale query scope;
- registry-backed record and query validation;
- deterministic link graph and field paths;
- versioned keyset cursors;
- incremental ingestion and inbox deduplication;
- PostgreSQL storage and distributed coordination;
- SQL planning/compilation;
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
M0 removed the complete source-specific implementation and its migrations,
contracts, runtime scheduler, server wiring, and admin table reads.

## Implemented core

M1 provides:

- bounded lowercase schema identifiers;
- ICU4X locale parsing and UTS #35/CLDR alias canonicalization;
- stable SHA-256 schema fingerprints;
- atomic versioned schema registration with idempotency and conflict detection;
- link target/type/cardinality validation;
- deterministic shortest-path graph resolution through `petgraph`;
- typed root and linked field paths;
- explicit tenant and locale query scope;
- select/filter/order/operator/type validation and bounded query complexity;
- rejection of ambiguous ordering through `many` links;
- checksummed postcard/Base64 keyset cursors bound to query scope and schema
  fingerprint;
- a test-only mutation/query reference engine and property invariants for later
  PostgreSQL equivalence testing.

## Status

- Rewrite: `in_progress`
- Current milestone: `M2 - PostgreSQL storage benchmark`
- FFA: `in_progress`
- FBA: `in_progress`
- M0 code reset: `complete`
- M1 generic core: `complete`
- Production migrations: intentionally absent pending M2 benchmark evidence

## Verification

The repository owner runs the checks during this rewrite:

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
