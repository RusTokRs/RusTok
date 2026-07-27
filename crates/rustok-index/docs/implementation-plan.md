# Implementation plan for `rustok-index`

## Mission

`rustok-index` is the platform-owned cross-module relational index and query
engine. Source modules publish generic schemas, records, mutations, and links;
Index materializes them into optimized relational storage and executes filtering,
projection, sorting, counting, and pagination without runtime fan-out to source
modules.

`rustok-index` is not a search engine. Ranking, relevance, typo tolerance,
synonyms, autocomplete, search UX, and external search-engine connectors remain
owned by `rustok-search`.

## Scope

This plan covers the generic schema and link registry, validated records and
mutations, PostgreSQL persistence, query planning/execution, incremental
ingestion, rebuild/reconciliation, operator controls, and the first owner-published
vertical slices. It excludes text relevance, ranking, autocomplete, external
search engines, source-table reads, and source-domain semantics in Index core.

Detailed completed-work evidence belongs in the accepted ADRs, committed evidence
packets, and Git history. This document remains the live roadmap and verification
contract.

## Rewrite policy

The project is in early development. **Backward compatibility with the rejected
implementation is not a goal.** Existing code, migrations, APIs, ports,
adapters, tests, fixtures, evidence, and documentation may be deleted or
replaced whenever they conflict with the target architecture.

## Update rules

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
Commits and pull requests record which checks and evidence runs were not executed.

## Current state

- Rewrite status: `in_progress`
- Current milestone: `M3 - PostgreSQL storage engine`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: `complete`
- M1 domain/application core: `complete`
- M2 storage benchmark: `complete`
- M2 storage decision: `JSONB accepted; rejected prototypes removed`
- M3 storage-schema foundation: `complete`
- M3 atomic mutation persistence: `complete`
- M3 schema-application leases: `complete`
- M3 secondary-index lifecycle: `complete`
- Production persistence: mutation writes, schema coordination, and schema-derived
  secondary-index lifecycle implemented; query adapter and partition lifecycle not
  yet implemented

The active production crate contains the generic domain/application core, the M3
production migrations, an Index-owned transactional mutation adapter, a durable
schema-application lease store, and a schema-derived secondary-index manager.
Query adapters, partition lifecycle, batch ingestion, and PostgreSQL Testcontainers
evidence remain open. Benchmark DDL and generated evidence stay under
`ops/benches`, outside the production module.

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
  migrations/
  infrastructure/
    postgres/
      mutation_store.rs
      schema_lease.rs
      secondary_index.rs
    events/
    telemetry.rs
  api/
    query.rs
    admin.rs

ops/benches/src/index_storage/
  config.rs
  runner.rs
  mutation_runner.rs
  maintenance_runner.rs
  sql/
```

## Library decisions

Use existing workspace libraries where possible:

- `sea-orm` and SeaQuery for PostgreSQL connections, transactions, migrations,
  execution, and dynamic SQL;
- `tokio` and `futures-util` for bounded async work;
- `serde` and `postcard` for DTO/cursor serialization;
- `thiserror` for typed errors;
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
- [x] Remove legacy v1 ports, adapters, source-specific indexers, projections,
      migrations, runtime configuration, scheduler, server composition, and direct
      source-table reads.
- [x] Remove source-domain dependencies and add guards preventing their return.
- [x] Synchronize local and central module documentation.

M0 is complete. No compatibility contract exists for deleted Index v1 behavior.

### M1 - Domain core and schema registry

- [x] Add bounded identifiers, canonical locales, schema identities, and versions.
- [x] Add `IndexValue`, `IndexRecord`, `IndexMutation`, `IndexSchema`, and link
      metadata.
- [x] Add stable order-independent SHA-256 schema fingerprints.
- [x] Add atomic versioned schema registration and deterministic link-path
      resolution.
- [x] Validate records, mutations, query paths, operators, types, cardinality,
      tenant/locale scope, and complexity bounds.
- [x] Add versioned checksummed query-scoped keyset cursors.
- [x] Add a test-only reference mutation/query engine and property invariants.

M1 is complete. Product and SalesChannel are representable by ordinary generic
schemas and links without Product-specific Index code.

### M2 - PostgreSQL storage benchmark

- [x] Define the benchmark contract and keep candidate DDL outside production
      migrations.
- [x] Add deterministic `smoke`, `100k`, and `1m` dataset presets.
- [x] Prototype JSONB entity rows plus typed expression/GIN indexes.
- [x] Compare JSONB, typed EAV, and specialized hot projection using equal result
      digests, cardinality, planner/session metadata, WAL, buffers, relation size,
      churn, and VACUUM evidence.
- [x] Run and archive replacement 100k Product-locale row read, mutation, and
      maintenance evidence from the accepted same-commit packet.
- [x] Run and archive replacement 1m Product-locale row read, mutation, and
      maintenance evidence from the same accepted commit.
- [x] Compare warm/cold buffers, planner stability, execution latency, ingestion
      throughput, relation size, WAL, dead tuples, vacuum behavior, and operational
      complexity.
- [x] Record the selected model and rejected alternatives in an ADR.
- [x] Delete benchmark prototypes that are not selected.

M2 is complete. The remaining JSONB benchmark path is a selected-layout regression
harness; it is not production persistence and does not reopen the accepted storage
decision.

### M3 - PostgreSQL storage engine

- [x] Add canonical schema/entity/link/inbox/job/checkpoint/consistency migrations.
- [x] Add tenant/schema/entity/locale keys and source-version guards.
- [x] Add atomic entity/link upsert and delete transactions.
- [x] Add locking/leases for schema application.
- [x] Add secondary-index planning and lifecycle management.
- [ ] Add partition management after measured design evidence.
- [ ] Add PostgreSQL Testcontainers fixtures.
- [ ] Cover migration-from-zero, stale mutation, redelivery, rollback,
      concurrency, and tenant/locale isolation in PostgreSQL.

The first M3 slice registers the seven module-owned tables. Their keys lead with
tenant identity, preserve the complete schema/entity/locale shape, bind entities
and links to exact non-negative `DECIMAL(20,0)` source versions, and retain schema
fingerprints in entity foreign keys.

The second M3 slice publishes `PostgresMutationStore`. Every delivery is validated
through `SchemaRegistry`, bound to a SHA-256 payload identity, claimed through the
composite inbox key, serialized by a transaction-scoped entity advisory lock, and
applied atomically. Exact redelivery is a duplicate; stale versions are terminally
ignored; deletes write payload-free tombstones.

The third M3 slice publishes `PostgresSchemaLeaseStore`. It serializes exact
tenant/module/entity/schema-version application with a transaction-scoped advisory
lock, verifies the persisted active schema and fingerprint, records durable
`schema_apply` work in `index_jobs`, and returns `Busy` or `AlreadyApplied` when
appropriate. Expired work is reclaimed with incremented attempt fencing. Heartbeat,
success, and failure require the exact job, worker, attempt, running state, and an
unexpired lease.

The fourth M3 slice publishes `SecondaryIndexPlan` and
`PostgresSecondaryIndexManager`. Plans derive deterministic tenant- and
schema-fingerprint-bound indexes from filterable/sortable fields. Scalar fields use
partial typed B-tree expressions ordered by locale/value/entity identity;
filterable `many` fields use field-local JSONB containment GIN. Stable names bind
the complete definition hash. `secondary_index` jobs coordinate ensure, concurrent
reindex, and concurrent retirement with advisory locking, expiry reclaim,
heartbeats, attempt fencing, persisted schema validation, owner comments, and
PostgreSQL readiness/validity inspection. Expressions follow the production tagged
`IndexValue` payload through each field's `value` member. SQLite remains
contract-test-only; PostgreSQL concurrency and Testcontainers evidence remain open.

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

## Verification

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
node scripts/verify/index-storage-tooling.mjs contract
node scripts/verify/index-storage-tooling.mjs fixtures
node --test scripts/verify/compare-index-storage-evidence.test.mjs
```

Targeted M3 maintainer checks:

```bash
cargo check -p rustok-index --all-targets
cargo test -p rustok-index --lib
node scripts/verify/verify-index-mutation-storage.mjs
node scripts/verify/verify-index-schema-leases.mjs
node scripts/verify/verify-index-secondary-index-lifecycle.mjs
```

## Progress log

- 2026-07-23: completed the destructive reset and generic M1 core.
- 2026-07-24 through 2026-07-27: completed the deterministic M2 PostgreSQL
  comparison, archived same-commit replacement evidence, accepted JSONB, and
  removed rejected prototypes.
- 2026-07-27: registered the canonical M3 storage schema and added atomic
  mutation/inbox/entity/link persistence.
- 2026-07-27: added durable schema-application exclusion, expiry reclaim,
  heartbeat, terminal completion, and attempt fencing.
- 2026-07-27: added schema-derived typed/containment secondary-index planning,
  concurrent ensure/reindex/retire execution, catalog readiness checks, durable
  jobs, owner fingerprints, and operation fencing. Tests and verifiers were left
  for the repository owner to execute.
