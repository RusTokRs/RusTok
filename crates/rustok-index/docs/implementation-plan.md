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
implementation is not a goal.** Existing code, migrations, APIs, ports,
adapters, tests, fixtures, evidence, and documentation may be deleted or
replaced whenever they conflict with the target architecture.

Rules:

1. Prefer a clean replacement over a compatibility layer.
2. Do not preserve placeholder APIs or tests that encode rejected architecture.
3. Index core must never query source-module tables directly.
4. Product, Content, Flex, Pricing, Inventory, and other source semantics must
   not be hard-coded in the generic engine.
5. Every completed task is checked off here in the same change.
6. Public boundary changes update local docs, the module manifest, central
   registry, verification scripts, and architecture decisions.
7. A milestone is complete only when its acceptance criteria are satisfied.

The repository owner performs test execution during this rewrite. Commits still
record which checks were not run.

## Current status

- Rewrite status: `in_progress`
- Current milestone: `M0/M1 - runtime-tail removal and domain core`
- FFA status: `in_progress`
- FBA status: `in_progress`
- Legacy `index.read_model.v1` and `index.rebuild.v1`: **deleted**
- Legacy source-specific indexers and migrations: **deleted**

The active crate consists of the generic domain core, module metadata, and one
temporary compatibility tail in `traits.rs`. That tail exists only because the
server still inserts `IndexerRuntimeConfig`; it is the final code-removal task
in M0.

## Ownership

`rustok-index` owns schema/link registration, generic records and mutations,
ingestion, inbox deduplication, relational storage, query validation/planning,
SQL compilation, filtering, projection, sorting, counting, pagination, rebuild,
checkpointing, reconciliation, drift repair, distributed coordination, and
operator diagnostics.

Source modules own normalized domain data, schema declarations, conversion to
generic Index records/mutations, paginated rebuild scan/load adapters, and
source ordering/version information.

`rustok-search` owns text relevance, ranking, typo tolerance, synonyms,
autocomplete, search UX, external search engines, and search-specific result
enrichment through stable Index contracts.

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

```text
src/
  domain/
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

Use existing workspace libraries where possible:

- `sea-orm` and SeaQuery for connections, transactions, migrations, execution,
  and dynamic SQL;
- `tokio` and `futures-util` for bounded async work;
- `serde` and `postcard` for DTO/cursor serialization;
- `thiserror` for typed errors;
- `validator` for external DTO/config validation only;
- `tracing`, `rustok-telemetry`, and `prometheus` for observability;
- `proptest` and `criterion` for invariants and benchmarks;
- `moka` only for immutable schema/compiled-plan local caching.

Add when required:

- `petgraph` for schema/link planning;
- `icu_locale_core` for locale canonicalization;
- `tokio-util` for cancellation/task tracking;
- `backon` for classified retries;
- `testcontainers-modules` with PostgreSQL;
- `insta` for plan/SQL/schema snapshots.

Forbidden in Index core:

- ranking/search-engine libraries;
- a second ORM/database stack;
- custom graph, locale, retry, or executor implementations;
- collecting all rebuild IDs in memory;
- source-table reads;
- source-domain crate dependencies;
- unvalidated JSON-only public queries.

## Milestones

### M0 - Hard reset and architecture lock

- [x] Replace the implementation plan with the Index Engine roadmap.
- [x] Record rewrite policy and target ownership in an ADR.
- [x] Reset local FBA readiness to `in_progress`.
- [x] Update crate/module documentation.
- [x] Inventory the active legacy surface.
- [x] Remove empty Content/Product query services.
- [x] Remove duplicate read adapters.
- [x] Remove `DocumentType` and `IndexDocument`.
- [x] Delete legacy v1 ports, registry, and evidence.
- [x] Delete the search-specific FTS helper.
- [x] Detach Index admin from legacy index/search tables.
- [x] Delete Content/Product/Flex indexers and projection models.
- [x] Remove all direct source-table SQL from Index.
- [x] Delete all legacy Index migrations, including the misplaced search table.
- [x] Remove source-domain Cargo dependencies.
- [x] Add repository verification preventing legacy/source-domain artifacts from
      returning.
- [ ] Remove `Indexer`, `LocaleIndexer`, `IndexerContext`,
      `IndexerRuntimeConfig`, and the old bounded scheduler.
- [ ] Remove the corresponding server dispatcher configuration/metrics.
- [ ] Synchronize the central module registry and historical FBA overview.

Done when the crate contains only generic engine code and intentional module
metadata, with no compatibility runtime tail.

### M1 - Domain core and schema registry

- [x] Add strong module/schema/entity/field/link/locale/version identifiers.
- [x] Add `IndexValue`, `IndexRecord`, and `IndexMutation`.
- [x] Add `IndexSchema`, field/link metadata, and base validation.
- [x] Add `IndexQuery`, field paths, filter AST, ordering, and pagination.
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
- [ ] Benchmark 100k and 1m multi-tenant/multi-locale entities.
- [ ] Measure equality/range/multi-value/link filters, sorting, keyset pagination,
      ingestion throughput, index size, and write amplification.
- [ ] Record the selected model and rejected alternatives in an ADR.

### M3 - PostgreSQL storage engine

- [ ] Add canonical schema/entity/link/inbox/job/checkpoint/consistency migrations.
- [ ] Add tenant/schema/entity/locale keys and source-version guards.
- [ ] Add atomic entity/link upsert and delete transactions.
- [ ] Add locking/leases for schema application.
- [ ] Add partition and secondary-index management.
- [ ] Add PostgreSQL Testcontainers fixtures.
- [ ] Cover migration-from-zero, stale mutation, redelivery, rollback,
      concurrency, and tenant/locale isolation.

### M4 - Query engine v1

- [ ] Validate schemas, fields, links, filters, ordering, and complexity.
- [ ] Resolve link paths and produce deterministic plans.
- [ ] Compile plans through SeaQuery or controlled SQL.
- [ ] Support nested projection, filtering, sorting, exact count, and keyset
      pagination.
- [ ] Keep offset pagination bounded and compatibility-only.
- [ ] Add plan/SQL snapshots and reference-evaluator equivalence tests.

### M5 - Incremental ingestion

- [ ] Add source and mutation registries.
- [ ] Add inbox deduplication and monotonic source versions.
- [ ] Add batch transactions, retry classification, backoff, dead-letter state,
      and lag metrics.
- [ ] Protect against out-of-order update/delete delivery.
- [ ] Cover crash between commit and acknowledgement.

### M6 - Rebuild and reconciliation

- [ ] Add cursor-based `IndexSource::scan` and targeted `load`.
- [ ] Add durable jobs, checkpoints, leases, heartbeat, and ownership.
- [ ] Add bounded streaming; never collect all IDs first.
- [ ] Add cancellation, resume, dry-run, targeted/full/shadow rebuild.
- [ ] Add reconciliation and drift repair.
- [ ] Cover crash, lease expiry, restart, cancellation, and incremental/full
      rebuild equivalence.

### M7 - First vertical slice

Entities: Product, ProductVariant, SalesChannel.

- [ ] Register owner-published schemas and links.
- [ ] Implement mutations and rebuild sources.
- [ ] Support tenant, locale, status, projection, link filters, sorting, and
      cursor pagination.
- [ ] Move one Storefront query to Index.
- [ ] Prove no source-module filtering fan-out.

### M8 - Commerce scale slice

- [ ] Add Pricing, Inventory, Category, Collection, Tags, Region/Currency, and
      Marketplace Seller schemas/sources.
- [ ] Filter by price, stock, category, channel, and seller in one query.
- [ ] Add cardinality and load benchmarks.

### M9 - Content, Flex, and extension schemas

- [ ] Add Content, Pages, Blog, Forum, Taxonomy, SEO, and Flex schemas.
- [ ] Make Flex use ordinary dynamic schema/source registration.
- [ ] Prove a new module requires no Index-core code changes.

### M10 - Horizontal scaling

- [ ] Test multiple workers/server instances, concurrent schema application and
      rebuild, redelivery, slow sources, connection loss, tenant hotspots, and
      backpressure.
- [ ] Add graceful shutdown and task-ownership evidence.
- [ ] Split core/postgres/worker crates only when measurements justify it.

### M11 - Admin and cutover

- [ ] Expose schema, partition, lag, inbox, failure, rebuild, drift, and query
      diagnostics.
- [ ] Add rebuild/cancel/retry commands.
- [ ] Publish new FBA contracts and runtime evidence.
- [ ] Migrate consumers and delete final compatibility code.
- [ ] Promote FBA only after compiled/live evidence.

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

## Progress log

- 2026-07-23: accepted the destructive rewrite and added the initial generic
  domain core.
- 2026-07-23: deleted empty query services, catch-all document types, duplicate
  read adapters, legacy v1 ports/FBA evidence, and the FTS helper.
- 2026-07-23: detached admin from legacy index/search tables.
- 2026-07-23: deleted all Content/Product/Flex indexers, projection models,
  source SQL, and legacy migrations.
- 2026-07-23: removed `rustok-api`, `rustok-events`, and `rustok-product` from the
  Index crate dependencies and added guards against their return.
