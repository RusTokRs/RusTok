# Implementation plan for `rustok-index`

## Mission

`rustok-index` is the platform-owned cross-module relational index and query
engine. It solves the same problem class as the Medusa Index Module: source
modules publish generic schemas, records, mutations, and links; Index
materializes them into optimized relational storage and executes filtering,
projection, sorting, counting, and pagination without runtime fan-out to source
modules.

`rustok-index` is not a search engine. Ranking, relevance, typo tolerance,
synonyms, autocomplete, search UX, and external search-engine connectors remain
owned by `rustok-search`.

## Rewrite policy

The project is in early development. **Backward compatibility with the rejected
implementation is not a goal.** Existing code, migrations, public Rust APIs,
ports, adapters, tests, fixtures, evidence, and documentation may be deleted or
replaced whenever they conflict with the target architecture.

Rules:

1. Prefer a clean replacement over a compatibility layer.
2. Do not preserve placeholder APIs or tests that encode rejected architecture.
3. Index core must never query source-module tables directly.
4. Product, Content, Flex, Pricing, Inventory, and other source semantics must
   not be hard-coded in the generic engine.
5. Every completed task is checked off here in the same change.
6. Public boundary changes update local documentation, the module manifest, the
   central registry, verification scripts, and architecture decisions.
7. A milestone is complete only when its acceptance criteria are satisfied.

The repository owner performs test execution during this rewrite. Commits must
still document which checks were not run.

## Current status

- Rewrite status: `in_progress`
- Current milestone: `M0/M1 - hard reset and domain core`
- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Legacy `index.read_model.v1` and `index.rebuild.v1`: **deleted**

The generic domain core is present. Source-specific Content/Product/Flex
indexers and their legacy migrations remain temporarily and are the next M0
deletion target.

## Target ownership

`rustok-index` owns:

- schema and link registry;
- generic records, mutations, source contracts, and query contracts;
- incremental ingestion and inbox deduplication;
- relational index storage;
- validation, graph planning, SQL compilation, filtering, projection, sorting,
  counting, and pagination;
- rebuild, checkpointing, reconciliation, drift detection, and repair;
- distributed coordination for schema changes and rebuild jobs;
- health, lag, progress, failures, and operator diagnostics.

Source modules own:

- normalized domain data;
- index schema declarations;
- conversion from domain state/events into generic records and mutations;
- paginated scan/load adapters used by rebuild;
- source ordering/version information.

`rustok-search` owns:

- text relevance and ranking;
- typo tolerance, synonyms, and autocomplete;
- search UX;
- external search-engine adapters;
- search-specific enrichment through stable Index contracts.

## Target architecture

```text
source modules
    -> IndexSource / IndexMutation
    -> ingestion and rebuild engines
    -> PostgreSQL index storage
    -> schema/link registry and query planner
    -> SQL compiler
    -> IndexQueryPort
    -> server, storefront, admin, and rustok-search
```

Target layout:

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

### Existing workspace libraries

- `sea-orm` and its SeaQuery re-export: connections, transactions, migrations,
  statement execution, and dynamic SQL generation.
- `tokio`: async runtime; no custom executor.
- `futures-util`: bounded pipelines through `buffer_unordered`.
- `serde` and `postcard`: DTO and cursor serialization.
- `thiserror`: typed domain/application/infrastructure errors.
- `validator`: external DTO and configuration validation only.
- `tracing`, `rustok-telemetry`, and `prometheus`: observability.
- `proptest`: invariants and reference-model tests.
- `criterion`: planner/compiler/ingestion benchmarks.
- `moka`: local immutable schema and compiled-plan cache only.

### Add when required

- `petgraph`: schema/link graph validation and deterministic path planning.
- `icu_locale_core`: canonical locale identifiers.
- `tokio-util`: `CancellationToken` and `TaskTracker`.
- `backon`: classified retry with exponential backoff.
- `testcontainers-modules` with PostgreSQL support: database integration tests.
- `insta`: normalized plan, SQL, and schema snapshots.

### Explicitly forbidden in Index core

- Elasticsearch, Meilisearch, Tantivy, or ranking libraries;
- a second ORM/database stack alongside SeaORM;
- custom graph, locale, retry, or executor implementations;
- loading all rebuild IDs into memory;
- source-module table reads;
- unvalidated JSON-only public query contracts.

## Milestones

### M0 - Hard reset and architecture lock

Goal: remove the rejected architecture and establish the new source of truth.

- [x] Replace the implementation plan with the Index Engine roadmap.
- [x] Record rewrite policy and target ownership in an ADR.
- [x] Reset local FBA readiness to `in_progress`.
- [x] Update crate/module documentation for the Index Engine mission.
- [x] Inventory the active legacy surface.
- [x] Remove incomplete Content/Product query services.
- [x] Remove duplicate in-memory/in-process read adapters.
- [x] Remove `DocumentType` and the catch-all `IndexDocument`.
- [x] Delete legacy v1 read/rebuild ports, registry, and evidence.
- [x] Delete the search-specific FTS helper from Index.
- [x] Remove direct legacy index/search table reads from Index admin.
- [x] Add repository verification that prevents legacy v1 artifacts returning.
- [ ] Synchronize the central module registry and historical FBA overview.
- [ ] Delete Content/Product/Flex source-specific indexers.
- [ ] Remove `Indexer`, `LocaleIndexer`, `IndexerRuntimeConfig`, and old rebuild
      scheduling.
- [ ] Remove direct source-table SQL from engine-owned code.
- [ ] Remove or replace all legacy Index migrations.
- [ ] Remove source-domain dependencies from `rustok-index/Cargo.toml`.
- [ ] Add a repository boundary check forbidding future source-domain imports in
      engine core.

Done when the crate contains only the generic engine skeleton, intentional
module metadata, and no source-specific persistence/runtime implementation.

### M1 - Domain core and schema registry

Goal: implement a database-independent, strongly typed engine core.

- [x] Add strong module/schema/entity/field/link/locale/version identifiers.
- [x] Add `IndexValue`, `IndexRecord`, and `IndexMutation`.
- [x] Add `IndexSchema`, field metadata, link metadata, and base validation.
- [x] Add `IndexQuery`, field paths, filter AST, ordering, and pagination models.
- [ ] Add locale canonicalization.
- [ ] Add stable schema hashing/versioning.
- [ ] Add schema registry conflict/version rules.
- [ ] Add link graph validation and deterministic path resolution.
- [ ] Add an in-memory reference evaluator used only by tests.
- [ ] Add property tests for tenant isolation, locale normalization, mutation
      idempotency, cursor round-trip, and deterministic planning.

Done when Product and SalesChannel can be represented entirely through generic
schemas and records with no Product-specific engine code.

### M2 - PostgreSQL storage benchmark

- [ ] Prototype JSONB plus typed expression indexes.
- [ ] Prototype typed field-value storage.
- [ ] Prototype a specialized hot-entity projection.
- [ ] Benchmark 100k and 1m entities with multi-tenant/multi-locale data.
- [ ] Measure equality/range/multi-value/link filters, sorting, keyset pagination,
      ingestion throughput, index size, and write amplification.
- [ ] Record the selected model and rejected alternatives in an ADR.

Done when storage is selected from measured evidence.

### M3 - PostgreSQL storage engine

- [ ] Add canonical migrations for schemas, entities, links, inbox, jobs,
      checkpoints, and consistency state.
- [ ] Add tenant/schema/entity/locale keys and source-version guards.
- [ ] Add atomic entity/link upsert and delete transactions.
- [ ] Add PostgreSQL locking/leases for schema application.
- [ ] Add partition and secondary-index management.
- [ ] Add PostgreSQL Testcontainers fixtures.
- [ ] Cover migration-from-zero, stale mutations, redelivery, rollback,
      concurrency, and tenant/locale isolation.

### M4 - Query engine v1

- [ ] Validate schemas, fields, links, filters, ordering, and complexity limits.
- [ ] Resolve link paths through the registry graph.
- [ ] Produce deterministic query plans.
- [ ] Compile plans with SeaQuery or controlled SQL.
- [ ] Support projections, nested linked fields, filters, sorting, exact count,
      and keyset pagination.
- [ ] Keep offset pagination as bounded compatibility mode only.
- [ ] Add plan/SQL snapshots and reference-evaluator equivalence tests.

Done when Product filtered by SalesChannel executes as one Index query.

### M5 - Incremental ingestion

- [ ] Add source and mutation registries.
- [ ] Add inbox deduplication and monotonic source-version handling.
- [ ] Add batch application and transaction boundaries.
- [ ] Add classified retry, backoff, dead-letter state, and lag metrics.
- [ ] Protect against out-of-order update/delete delivery.
- [ ] Cover crash between commit and acknowledgement.

### M6 - Rebuild and reconciliation

- [ ] Add cursor-based `IndexSource::scan` and targeted `load`.
- [ ] Add durable jobs, checkpoints, leases, heartbeat, and worker ownership.
- [ ] Add bounded streaming; never collect all IDs first.
- [ ] Add cancellation, resume, dry-run, targeted/full/shadow rebuild.
- [ ] Add reconciliation and drift repair.
- [ ] Cover worker crash, lease expiry, restart, cancellation, and incremental vs
      full rebuild equivalence.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [ ] Register owner-published schemas and links.
- [ ] Implement mutations and rebuild sources.
- [ ] Support tenant, locale, status, projection, link filters, sorting, and
      cursor pagination.
- [ ] Move one Storefront query to Index.
- [ ] Prove there is no source-module filtering fan-out.

### M8 - Commerce scale slice

Order: Pricing, Inventory, Category, Collection, Tags, Region/Currency,
Marketplace Seller.

- [ ] Register schemas, records, mutations, and rebuild sources.
- [ ] Filter by price, stock, category, channel, and seller in one query.
- [ ] Add cardinality and load benchmarks.

### M9 - Content, Flex, and extension schemas

- [ ] Add Content, Pages, Blog, Forum, Taxonomy, SEO, and Flex schemas.
- [ ] Make Flex use ordinary dynamic schema/source registration.
- [ ] Prove a new module requires no Index-core code changes.

### M10 - Horizontal scaling

- [ ] Test multiple workers and server instances.
- [ ] Test concurrent schema application/rebuild, redelivery, slow sources,
      connection loss, tenant hotspots, and backpressure.
- [ ] Add graceful shutdown and task-ownership evidence.
- [ ] Split core/postgres/worker crates only when measurements justify it.

### M11 - Admin and cutover

- [ ] Expose schema, partition, lag, inbox, failure, rebuild, drift, and query
      diagnostics.
- [ ] Add rebuild/cancel/retry operator commands.
- [ ] Publish new FBA contracts and runtime evidence.
- [ ] Migrate consumers.
- [ ] Delete any final compatibility code.
- [ ] Promote FBA status only after compiled/live evidence passes.

## Quality gates

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo xtask module validate index
cargo xtask module test index
npm run verify:index:fba
npm run verify:index:runtime-fallback-smoke
```

Additional gates include PostgreSQL integration, migration-from-zero, property
tests, planner/compiler snapshots, rebuild equivalence, crash recovery,
concurrency, and benchmark evidence.

## Progress log

- 2026-07-23: accepted the destructive Index Engine rewrite and added the first
  database-independent domain types.
- 2026-07-23: deleted empty Content/Product query services, catch-all
  `IndexDocument`, duplicate read adapters, legacy v1 ports/FBA evidence, and the
  search-specific FTS helper.
- 2026-07-23: detached Index admin from `index_content`, `index_products`, and
  `search_index`; admin now reports the real rewrite state.
- 2026-07-23: converted Index verification scripts into guards preventing legacy
  v1 artifacts and direct legacy table reads from returning.
