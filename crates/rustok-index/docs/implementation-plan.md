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
8. Benchmark scaffolding is not production persistence and must not leak into
   `rustok-index` migrations or runtime composition.

The repository owner performs test and benchmark execution during this rewrite.
Commits record which checks and evidence runs were not executed.

## Current status

- Rewrite status: `in_progress`
- Current milestone: `M2 - PostgreSQL storage benchmark`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: `complete`
- M1 domain/application core: `complete`
- M2 benchmark harness: `implemented; evidence runs pending`
- Production persistence: intentionally absent until the M2 ADR selects a model

The active production crate contains only the generic domain/application core,
`IndexModule` metadata, and an intentionally empty migration source. Benchmark
DDL and generated evidence live under `ops/benches`, outside the production
module.

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
crates/rustok-index/src/
  domain/
  application/
  infrastructure/
    postgres/
    events/
    telemetry.rs
  api/
    query.rs
    admin.rs

ops/benches/src/index_storage/
  config.rs
  sql.rs
  runner.rs
```

## Library decisions

Use existing workspace libraries where possible:

- `sea-orm` and SeaQuery for PostgreSQL connections, transactions, migrations,
  execution, and dynamic SQL;
- `tokio` and `futures-util` for bounded async work;
- `serde` and `postcard` for DTO/cursor serialization;
- `thiserror` for typed errors;
- `validator` for external DTO/config validation only;
- `tracing`, `rustok-telemetry`, and `prometheus` for observability;
- `proptest` and `criterion` for invariants and benchmarks;
- `moka` only for immutable schema/compiled-plan local caching.

Selected additions:

- `petgraph` for deterministic schema/link graph traversal;
- `icu_locale` with compiled ICU4X data for UTS #35/CLDR locale alias
  canonicalization;
- `sha2` for stable schema fingerprints and cursor checksums;
- `postcard` plus URL-safe Base64 for versioned keyset cursors.

Add when required:

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
- [x] Delete the legacy runtime config, scheduler, and operational errors.
- [x] Remove legacy server dispatcher config and metrics.
- [x] Add repository guards preventing legacy/source-domain artifacts returning.
- [ ] Synchronize the central module registry and historical FBA overview.

The code acceptance criterion is complete. The remaining unchecked item is
cross-cutting documentation cleanup and does not preserve a runtime dependency.

### M1 - Domain core and schema registry

- [x] Add bounded lowercase identifiers for modules, schemas, entities, fields,
      links, locales, and versions.
- [x] Add `IndexValue`, `IndexRecord`, and `IndexMutation`.
- [x] Add `IndexSchema`, field/link metadata, and contract validation.
- [x] Add explicit tenant/locale query scope.
- [x] Add typed link-aware field paths, filter AST, ordering, and pagination.
- [x] Add ICU4X locale syntax and CLDR alias canonicalization.
- [x] Add stable order-independent SHA-256 schema fingerprints.
- [x] Add atomic versioned schema registration and conflict rules.
- [x] Validate link targets, target fields, join-field types, and cardinality.
- [x] Add deterministic shortest-path resolution through `petgraph`.
- [x] Validate records against registered schemas and locale/cardinality rules.
- [x] Validate query selectability, filterability, sortability, operators, types,
      scope, and complexity limits.
- [x] Reject ambiguous sorting through `many` links until aggregation is explicit.
- [x] Add versioned, checksummed, query-scoped keyset cursor encoding.
- [x] Add a test-only in-memory reference mutation/query engine.
- [x] Add property tests for tenant isolation, redelivery idempotency, stale
      tombstones, locale normalization, cursor round-trip, and deterministic
      planning.

Acceptance criterion is complete: Product and SalesChannel are representable by
ordinary generic schemas and links with no Product-specific engine code.

### M2 - PostgreSQL storage benchmark

Goal: select physical storage from evidence before creating production
migrations.

- [x] Define the benchmark contract and decision rules in
      `docs/storage-benchmark.md`.
- [x] Keep all candidate DDL outside production migrations in `ops/benches`.
- [x] Add deterministic `smoke`, `100k`, and `1m` dataset presets.
- [x] Canonicalize configured locales through `LocaleKey` before SQL generation.
- [x] Generate Product, Variant, SalesChannel, tags, prices, timestamps, and links
      without random or wall-clock inputs.
- [x] Prototype JSONB entity rows plus typed expression/GIN indexes.
- [x] Prototype normalized typed field-value rows.
- [x] Prototype a specialized hot typed projection as the comparison baseline.
- [x] Represent links independently from entity payload storage in every model.
- [x] Run the same equality, range, multi-value, two-hop link, keyset, and count
      workload definitions against every model.
- [x] Capture prototype load time, schema bytes, PostgreSQL settings, full SQL,
      and repeated `EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` output.
- [x] Add a release-mode executable that writes machine-readable JSON evidence.
- [ ] Run and archive the `smoke` evidence as a harness sanity check.
- [ ] Run and archive 100k Product-locale row evidence.
- [ ] Run and archive 1m Product-locale row evidence.
- [ ] Add update/delete workloads for write amplification, WAL, bloat, and vacuum
      impact.
- [ ] Compare warm/cold buffers, planner stability, execution latency, ingestion
      throughput, relation size, and operational complexity.
- [ ] Record the selected model and rejected alternatives in an ADR.
- [ ] Delete benchmark prototypes that are not selected.

M2 remains open until real PostgreSQL evidence is archived and the storage ADR
is accepted. Implementing the harness does not select a model.

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

- [ ] Produce deterministic executable query plans from validated queries.
- [ ] Resolve explicit link paths and assign stable aliases.
- [ ] Compile plans through SeaQuery or controlled SQL.
- [ ] Support nested projection, filtering, sorting, exact count, and keyset
      pagination.
- [ ] Keep offset pagination bounded and compatibility-only.
- [ ] Add plan/SQL snapshots and PostgreSQL/reference-engine equivalence tests.

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

M2 operational commands:

```bash
DATABASE_URL=postgres://... INDEX_BENCH_SCALE=smoke \
  cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
DATABASE_URL=postgres://... INDEX_BENCH_SCALE=100k \
  cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
DATABASE_URL=postgres://... INDEX_BENCH_SCALE=1m \
  cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
```

## Progress log

- 2026-07-23: accepted the destructive rewrite and added the initial generic
  domain core.
- 2026-07-23: deleted every legacy query, projection, indexer, migration, port,
  adapter, scheduler, runtime config, error type, and server composition path.
- 2026-07-23: completed M1 with canonical identifiers/locales, schema
  fingerprints, atomic registry, deterministic link graph, scoped validation,
  bounded queries, keyset cursors, reference evaluator, and property invariants.
- 2026-07-23: moved the active milestone to the PostgreSQL storage benchmark;
  no production persistence will be added before the benchmark ADR.
- 2026-07-23: implemented the M2 deterministic dataset, JSONB/EAV/hot candidates,
  independent link storage, shared workload suite, PostgreSQL runner, and JSON
  EXPLAIN evidence format. Real database runs and the storage ADR remain open.
