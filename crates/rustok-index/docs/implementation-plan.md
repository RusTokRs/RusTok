# Implementation plan for `rustok-index`

## Mission

`rustok-index` is the platform-owned cross-module relational index and query
engine. Its functional target is the same problem class as the Medusa Index
Module: source modules publish indexable schemas, records, and links; Index
materializes them into an optimized relational store and executes filtering,
projection, sorting, counting, and pagination without runtime fan-out to the
source modules.

`rustok-index` is not a search engine. Ranking, typo tolerance, relevance,
autocomplete, synonyms, and external search-engine connectors remain owned by
`rustok-search`.

## Rewrite policy

The module is in early development. Backward compatibility with the current
internal implementation is not a goal during this rewrite.

The implementation may remove or replace any existing code, migration, public
Rust API, port, adapter, test, fixture, or documentation when it conflicts with
the target architecture. Reuse is allowed only when the existing code fits the
new contracts and remains simpler than a replacement.

Rules:

1. Prefer a clean replacement over compatibility layers.
2. Do not preserve placeholder APIs or tests that encode rejected architecture.
3. Do not let Index query source-module tables directly.
4. Do not hard-code Product, Content, or Flex behavior in the engine core.
5. Every completed task is checked off in this document in the same PR.
6. Every public contract change updates local docs, `rustok-module.toml`, the
   central module registry, contract evidence, and relevant architecture docs.
7. A milestone is complete only when its acceptance criteria and required tests
   pass.

## Current status

- Rewrite status: `in_progress`
- Current milestone: `M0 - hard reset and architecture lock`
- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Legacy FBA contracts: `index.read_model.v1` and `index.rebuild.v1`; retained
  temporarily only until the new query/rebuild boundary is implemented.

The existing module is a source-specific CQRS/read-model implementation with
hard-coded Content, Product, and Flex indexers, source-table SQL, duplicate
in-memory adapters, incomplete query services, and rebuild orchestration that
collects entity IDs before scheduling work. These parts are legacy and may be
deleted rather than incrementally preserved.

## Target ownership

`rustok-index` owns:

- index schema registry;
- field and link metadata;
- source registration contracts;
- incremental ingestion and inbox deduplication;
- relational index storage;
- query validation, planning, and SQL compilation;
- filtering, projection, sorting, count, and pagination;
- rebuild, checkpointing, reconciliation, and drift detection;
- distributed coordination for schema changes and rebuild jobs;
- index health, lag, progress, and operator diagnostics.

Source modules own:

- their normalized domain data;
- their index schema declarations;
- conversion from domain data/events to index records and mutations;
- scan/load adapters used by rebuild;
- semantic version/order information for mutations.

`rustok-search` owns:

- text relevance and ranking;
- typo tolerance and synonyms;
- autocomplete and search UX;
- external search-engine adapters;
- search-specific result enrichment through stable Index contracts.

## Target architecture

```text
source modules
    -> IndexSource / IndexMutation contracts
    -> ingestion and rebuild engines
    -> PostgreSQL index storage
    -> query validator and link-graph planner
    -> SQL compiler
    -> IndexQueryPort
    -> server, storefront, admin, and rustok-search
```

The engine core must not depend on Product, Content, Flex, Pricing, Inventory,
or other source crates.

Proposed crate layout:

```text
src/
  domain/
    identifiers.rs
    value.rs
    schema.rs
    record.rs
    mutation.rs
    query.rs
    error.rs
  application/
    registry.rs
    ingestion.rs
    rebuild.rs
    reconcile.rs
    ports.rs
  infrastructure/
    postgres/
    events/
    telemetry.rs
  api/
    query.rs
    admin.rs
```

## Library decisions

### Use existing workspace libraries

- `sea-orm` and its re-exported SeaQuery: connection management,
  transactions, migrations, statement execution, and dynamic SQL generation.
- `tokio`: async runtime only; no custom executor.
- `futures-util`: bounded async pipelines through `buffer_unordered` where
  separate task ownership is unnecessary.
- `serde` and `postcard`: stable internal DTO/cursor serialization.
- `thiserror`: typed domain/application/infrastructure errors.
- `validator`: external DTO and configuration validation only.
- `tracing`, `rustok-telemetry`, and `prometheus`: spans, metrics, and
  operational evidence.
- `proptest`: invariants and reference-model tests.
- `criterion`: planner/compiler/ingestion benchmarks.
- `moka`: local immutable schema and compiled-plan cache only.

### Add during implementation

- `petgraph`: schema/link graph validation and query-path planning.
- `icu_locale_core`: canonical locale identifiers used in keys and queries.
- `tokio-util`: `CancellationToken` and `TaskTracker` for rebuild lifecycle.
- `backon`: classified retry with exponential backoff.
- `testcontainers-modules` with PostgreSQL support: real database integration
  tests.
- `insta`: snapshots for normalized query plans, generated SQL, and schema
  manifests.

### Explicitly avoid in Index core

- Elasticsearch, Meilisearch, Tantivy, or search ranking libraries;
- a second ORM or direct database stack alongside SeaORM;
- a custom graph implementation;
- a custom locale parser;
- a custom retry/backoff implementation;
- loading all rebuild IDs into memory;
- direct reads from source-module tables;
- JSON-only unvalidated public query contracts.

## Milestones

### M0 - Hard reset and architecture lock

Goal: remove the rejected architecture and establish the new source of truth.

- [x] Replace the live implementation plan with the Index Engine roadmap.
- [x] Record the rewrite policy and target ownership in an ADR.
- [x] Reset FBA readiness from `boundary_ready` to `in_progress`.
- [x] Update crate/module documentation to describe the Index Engine mission.
- [ ] Inventory legacy files and classify each as delete, migrate, or reuse.
- [ ] Remove incomplete source-specific query services.
- [ ] Remove duplicate in-memory/in-process adapters.
- [ ] Remove `DocumentType` and the canonical `IndexDocument` catch-all model.
- [ ] Remove direct source-table SQL from engine-owned code.
- [ ] Remove or replace legacy index migrations.
- [ ] Add repository checks forbidding new source-module dependencies in the
  engine core.

Done when the crate contains only an intentional compatibility shell plus the
new engine skeleton, and all deleted APIs have corresponding documentation and
registry updates.

### M1 - Domain core and schema registry

Goal: implement a database-independent, strongly typed engine core.

- [ ] Add strong identifiers for modules, schemas, entities, fields, links,
  locales, and schema versions.
- [ ] Add `IndexValue`, `IndexRecord`, and `IndexMutation`.
- [ ] Add `IndexSchema`, field metadata, link metadata, and schema validation.
- [ ] Add `IndexQuery`, selected field paths, filter AST, ordering, and cursor
  pagination models.
- [ ] Add locale canonicalization.
- [ ] Add schema hashing/versioning.
- [ ] Add link graph validation and deterministic path resolution.
- [ ] Add an in-memory reference evaluator used only by tests.
- [ ] Add property tests for tenant isolation, locale normalization, mutation
  idempotency, cursor round-trip, and deterministic planning.

Done when Product and SalesChannel can be represented entirely through generic
schemas and records without Product-specific code in the engine.

### M2 - PostgreSQL storage benchmark

Goal: choose the physical storage model using measured evidence.

- [ ] Prototype JSONB plus typed expression indexes.
- [ ] Prototype typed field-value storage.
- [ ] Prototype a specialized hot-entity projection.
- [ ] Benchmark 100k and 1m entities with multi-tenant/multi-locale data.
- [ ] Benchmark equality/range/multi-value/link filters, sorting, keyset
  pagination, ingestion throughput, index size, and write amplification.
- [ ] Record the selected model and rejected alternatives in an ADR.

Done when storage is selected from benchmark evidence rather than preference.

### M3 - PostgreSQL storage engine

- [ ] Add canonical migrations for schemas, entities, links, inbox, jobs,
  checkpoints, and consistency state.
- [ ] Add tenant/schema/entity/locale keys and source-version guards.
- [ ] Add atomic entity/link upsert and delete transactions.
- [ ] Add schema apply coordination through PostgreSQL locking/leases.
- [ ] Add partition and secondary-index management.
- [ ] Add Testcontainers PostgreSQL fixtures.
- [ ] Test migration-from-zero, stale mutations, redelivery, rollback,
  concurrency, and tenant/locale isolation.

Done when all persistence semantics pass against real PostgreSQL.

### M4 - Query engine v1

- [ ] Validate schemas, fields, links, filters, ordering, and complexity limits.
- [ ] Resolve link paths through the registry graph.
- [ ] Produce deterministic query plans.
- [ ] Compile plans with SeaQuery/controlled SQL.
- [ ] Support projection, nested linked fields, filters, sorting, exact count,
  and keyset pagination.
- [ ] Keep offset pagination as bounded compatibility mode only.
- [ ] Add SQL and plan snapshots.
- [ ] Add a reference-evaluator equivalence test suite.

Done when a Product filtered by SalesChannel executes as one Index storage
query without source-module fan-out.

### M5 - Incremental ingestion

- [ ] Add source and mutation registries.
- [ ] Add inbox deduplication and monotonic source-version handling.
- [ ] Add batch application and transaction boundaries.
- [ ] Add retry classification, backoff, dead-letter state, and lag metrics.
- [ ] Protect against out-of-order update/delete delivery.
- [ ] Test crash between commit and acknowledgement.

Done when repeated and out-of-order event delivery produces a correct final
index state.

### M6 - Rebuild and reconciliation engine

- [ ] Add cursor-based `IndexSource::scan` and targeted `load` contracts.
- [ ] Add durable jobs, checkpoints, leases, heartbeat, and worker ownership.
- [ ] Add bounded streaming pipelines; never collect all IDs first.
- [ ] Add cancellation, resume, dry-run, targeted rebuild, full rebuild, and
  shadow rebuild.
- [ ] Add reconciliation and drift repair.
- [ ] Test worker crashes, lease expiry, restart, cancellation, and equivalent
  incremental/full rebuild results.

Done when rebuild resumes after process failure and multiple workers cannot
process the same batch concurrently.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [ ] Register schemas and links from their owner modules.
- [ ] Implement incremental mutations and rebuild sources.
- [ ] Support tenant, locale, status, fields, link filters, sorting, and cursor
  pagination.
- [ ] Move one Storefront query to the Index query contract.
- [ ] Prove no source-module filtering fan-out occurs.

Done when published products for a tenant and sales channel are returned from
one Index query.

### M8 - Commerce scale slice

Order: Pricing, Inventory, Category, Collection, Tags, Region/Currency,
Marketplace Seller.

- [ ] Register schemas, records, mutations, and rebuild sources.
- [ ] Support price, stock, category, channel, and seller filtering in one
  query.
- [ ] Add load and cardinality benchmarks for commerce links.

### M9 - Content, Flex, and extension schemas

- [ ] Add Content, Pages, Blog, Forum, Taxonomy, SEO projections, and Flex.
- [ ] Make Flex use ordinary dynamic schema/source registration.
- [ ] Ensure adding a new module requires no Index-core code changes.

### M10 - Horizontal scaling and extraction proof

- [ ] Test multiple index workers and server instances.
- [ ] Test concurrent schema apply, rebuild, event redelivery, slow sources,
  connection loss, tenant hotspots, and backpressure.
- [ ] Add graceful shutdown and task ownership evidence.
- [ ] Split `rustok-index-core`, `rustok-index-postgres`, or worker crates only
  when measured boundaries justify the split.

### M11 - Admin, cutover, and legacy removal

- [ ] Expose schema, partition, lag, inbox, failed mutation, rebuild, drift, and
  query diagnostics.
- [ ] Add rebuild/cancel/retry operator commands.
- [ ] Publish the new FBA contracts and runtime evidence.
- [ ] Migrate consumers.
- [ ] Delete all legacy ports, adapters, migrations, and compatibility code.
- [ ] Promote FBA status only after compiled/live evidence passes.

## Required quality gates

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo xtask module validate index
cargo xtask module test index
npm run verify:index:fba
npm run verify:foundation:fba-runtime-smoke
```

Additional gates:

- PostgreSQL integration suite;
- migration-from-zero;
- property tests;
- planner/compiler snapshots;
- rebuild equivalence tests;
- concurrency and crash-recovery tests;
- benchmark evidence for storage and hot query paths;
- no unjustified `todo!`, `unimplemented!`, `#[ignore]`, `unwrap`, or `expect`
  in runtime paths.

## Progress log

- 2026-07-23: rewrite approved; module target redefined as a generic
  cross-module relational Index Engine; destructive replacement explicitly
  allowed; M0 started; documentation/ADR/status reset initiated.
