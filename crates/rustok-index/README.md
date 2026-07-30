# rustok-index

## Purpose

`rustok-index` is RusToK's cross-module relational Index Engine. Source modules
publish generic schemas, records, mutations, and links; Index materializes them
into optimized storage and executes filtering, projection, sorting, counting,
and pagination without runtime fan-out to source modules.

Backward compatibility with the rejected source-specific implementation is not
a rewrite goal.

## Responsibilities

- Own the generic schema and link registry.
- Own incremental ingestion, deduplication, rebuild, reconciliation, and drift
  control.
- Own PostgreSQL index storage and distributed coordination.
- Validate and plan cross-module queries.
- Compile projection, filtering, sorting, count, and pagination to Index storage
  queries.
- Execute compiled queries through an Index-owned PostgreSQL port.
- Publish stable query, source, rebuild, and operator contracts.
- Keep product-facing relevance and ranking in `rustok-search`.

## Boundaries

- Index core must not depend on Product, Content, Flex, Pricing, Inventory, or
  other source-domain crates.
- Source modules own conversion from domain state/events into generic records and
  mutations.
- Index must not read source-module tables directly.
- `rustok-search` owns ranking, typo tolerance, autocomplete, synonyms, search
  UX, and external search-engine connectors.
- The selected JSONB regression DDL lives under `ops/benches`; it is not a
  production migration or runtime storage contract. Historical three-candidate
  evidence is archived under `docs/evidence/`.

## Rewrite status

- Current milestone: `M4 - Query engine v1`
- FFA status: `in_progress`
- FBA status: `in_progress`
- M0 code reset: complete
- M1 generic domain/application core: complete
- M2 PostgreSQL storage benchmark: complete
- M2 accepted storage model: JSONB
- M2 rejected prototype cleanup: complete
- M3 storage-schema foundation: complete
- M3 atomic mutation persistence: complete
- M3 schema-application leases: complete
- M3 secondary-index lifecycle: complete
- M3 partition admission and shadow planning: complete
- M3 partition evidence capture and packet assembly: complete
- M3 retained packet owner orchestration: complete
- M3 retained bundle review and archive verification: complete
- M4 deterministic query planning and controlled SQL compilation: complete
- M4 root/one and many-link query result semantics: complete
- M4 PostgreSQL query port and row adapter: source complete
- M4 source-owned registry and server query-runtime composition: source complete
- Real retained PostgreSQL packet execution: open
- Query-port authorization/consumer cutover and live equivalence evidence: open

All legacy ports, adapters, source indexers, projections, migrations, runtime
configuration, scheduler, and errors from the rejected source-specific design remain
deleted. M3 registers the canonical production schema, publishes an Index-owned
transactional mutation adapter, owns durable schema-application leases, manages
deterministic schema-derived secondary indexes, and rejects partition rollout until
measured shadow evidence passes an explicit policy. Owner-operated tooling captures
and validates the nine-file retained bundle, renders a read-only review, emits an
admitted-only archive manifest outside the bundle, and verifies the saved manifest
against an exact recursive filesystem snapshot.

M4 now provides deterministic typed planning, controlled PostgreSQL compilation,
correlated many-link filtering, nested many-link projection aggregation, strict
result decoding, lookahead pagination, exact count, scoped cursors, an Index-owned
PostgreSQL execution port, a source-owned immutable schema catalog, and a server-owned
neutral query runtime. Storefront/admin/search authorization and consumer cutover,
many-link aggregate ordering policy, live PostgreSQL/reference equivalence, and
production partition cutover remain open.

The module-owned migration source creates:

- `index_schemas` for versioned owner-published schema contracts;
- `index_entities` for the tenant-leading JSONB entity envelope and full-range `DECIMAL(20,0)` source version;
- `index_links` for independently relational ordered links;
- `index_inbox` for durable delivery deduplication and leases;
- `index_checkpoints` for ingestion/rebuild cursors;
- `index_jobs` for schema, index, rebuild, reconciliation, and consistency work;
- `index_consistency_findings` for durable drift diagnostics.

The `PostgresMutationStore` applies one `MutationDelivery` transactionally. It
validates the mutation through `SchemaRegistry`, reserves the tenant-scoped inbox
identity, rejects delivery-ID payload reuse, takes a transaction-scoped advisory
lock on the complete entity key, reads the current source version, and either
terminally ignores a stale delivery or replaces the live JSONB fields and ordered
links with the incoming state. Deletes write a tombstone. The inbox row is
completed in the same commit. Exact redelivery returns
`MutationApplyOutcome::Duplicate`; a failed entity/link write rolls back the inbox
claim. SQLite support exists only for contract fixtures and rejects source versions
above its signed integer range; PostgreSQL preserves the full domain `u64` range
through `DECIMAL(20,0)`.

The `PostgresSchemaLeaseStore` coordinates one `schema_apply` job per exact
tenant/module/entity/schema-version scope. Acquisition is serialized with a
transaction-scoped PostgreSQL advisory lock, verifies the persisted schema and
fingerprint, returns `Busy` while another non-expired owner holds the lease, and
returns `AlreadyApplied` after terminal success. Expired work is reclaimed with an
incremented attempt fence; heartbeat, success, and failure require the exact job,
worker, attempt, running state, and unexpired lease so an old owner cannot commit
after takeover. SQLite support remains contract-test-only.

`SecondaryIndexPlan` derives one deterministic index specification for every
filterable or sortable field. Scalar fields use typed partial B-tree expressions;
filterable `many` fields use field-local JSONB containment GIN. Expressions follow
the production tagged `IndexValue` payload contract through each field's `value`
member. Stable names bind tenant, schema reference, schema fingerprint, field type,
cardinality, and index kind. `PostgresSecondaryIndexManager` coordinates ensure,
reindex, and retirement through fenced `secondary_index` jobs, executes PostgreSQL
`CONCURRENTLY` DDL, records owner definition comments, and verifies `indisready`
and `indisvalid` before completion. Retire remains available for retired schemas;
SQLite is contract-test-only.

`evaluate_partition_admission` keeps the canonical tables unpartitioned unless one
explicit policy is satisfied by one exact SHA-256 evidence packet. Admission checks
minimum measured size/row/tenant scale, tenant-predicate coverage, entity and link
digest parity, shadow catch-up, foreign-key validation, orphan-link absence, query
plan stability, p95 query/mutation regressions, WAL amplification, partition-size
skew, and cutover-lock duration. An admitted `PartitionShadowPlan` derives stable
hash-partition parent/child names and PostgreSQL bootstrap statements for shadow
relations only. It never emits production table rename, drop, or cutover SQL.
Tenant-hash modulus must be a power of two from 2 through 128. Actual copy,
constraint/index attachment, dual-write/replay, cutover, rollback, and durable
global operation ownership remain later work and require retained PostgreSQL
evidence.

The retained evidence boundary is owner-operated and read-only after capture. Review
recalculates packet assembly and admission from exactly nine authoritative files.
Archive verification rereads files and the saved external manifest through stable
descriptors, checks filesystem identity and metadata fingerprints, verifies the
exact recursive directory inventory, and fails closed on aliases, replacement,
metadata drift, inventory drift, or byte drift. Public manifest and receipt schemas
remain stable, and every successful receipt keeps
`production_lifecycle_authorized: false`.

`IndexQueryPort` is the transport-neutral query execution boundary.
`PostgresIndexQueryPort` owns a PostgreSQL connection and immutable
`Arc<SchemaRegistry>`. Each call compiles through the registry, starts one read-only
repeatable-read transaction, verifies every root/source/target schema against the
query tenant's exact active `index_schemas` fingerprint and JSON contract, executes
the page and optional exact-count statements in the same snapshot, maps only
compiler-declared UUID/JSONB/bigint aliases, and delegates semantic validation and
cursor construction to `decode_postgres_query_page`. Authentication and transport
policy remain caller responsibilities.

`IndexSchemaSourceCatalog` collects owner-published generic contracts during module
registration and fixes one owner for each complete schema identity across versions.
`rustok-distribution` materializes all entries through one atomic batch and publishes
`SharedIndexSchemaRegistry` only for a non-empty catalog. The server then calls the
Index-owned `materialize_postgres_index_query_runtime`, which binds that exact immutable
registry to the host database and publishes `SharedIndexQueryRuntime` through
`ModuleRuntimeExtensions`. Composition performs no SQL and does not claim tenant schema
readiness; execution still fails closed through the query-port preflight.

## Current entry points

- `IndexModule`
- `rustok_index::domain::*`
- `rustok_index::application::*`
- `rustok_index::migrations::*`
- `PostgresMutationStore`, `MutationDelivery`, and `MutationApplyOutcome`
- `PostgresSchemaLeaseStore`, `SchemaApplicationLeaseRequest`,
  `SchemaApplicationLease`, and `SchemaLeaseAcquireOutcome`
- `SecondaryIndexPlan`, `SecondaryIndexSpec`, `SecondaryIndexRequest`,
  `SecondaryIndexLease`, and `PostgresSecondaryIndexManager`
- `PartitionAdmissionPolicy`, `PartitionEvidence`, `PartitionAdmissionOutcome`,
  `PartitionShadowPlan`, and `evaluate_partition_admission`
- `SchemaRegistry`, `IndexSchema`, `IndexRecord`, and `IndexMutation`
- `IndexSchemaSourceCatalog`, `SharedIndexSchemaRegistry`, and
  `register_index_schema_source`
- `IndexQuery`, `IndexQueryScope`, `FilterExpr`, and typed `FieldPath`
- `ExecutableQueryPlan`, `CompiledPostgresQuery`, and `CompiledPostgresPageQuery`
- `IndexQueryPort`, `PostgresIndexQueryPort`, `SharedIndexQueryRuntime`,
  `materialize_postgres_index_query_runtime`, `IndexQueryExecutionError`, and
  `IndexQueryPage`
- `CursorCodec`, `IndexCursor`, and query-scope cursor validation

## Implemented invariants

- bounded lowercase identifiers;
- ICU4X syntax and CLDR alias locale canonicalization;
- stable order-independent schema fingerprints;
- atomic versioned schema registration;
- deterministic link-path resolution;
- tenant/locale-scoped records and queries;
- registry-backed type, cardinality, field, link, and operator validation;
- bounded query complexity and pagination;
- no ambiguous ordering through `many` links;
- checksummed keyset cursors bound to tenant, schema, fingerprint, locale, and
  order shape;
- reference mutation/query engine and property-based invariants for future
  PostgreSQL equivalence tests;
- atomic tenant-scoped inbox/entity/link mutation persistence with monotonic
  source-version and tombstone admission;
- durable schema-application exclusion, expiry reclaim, heartbeat, terminal
  completion, and attempt fencing;
- deterministic tenant/schema/fingerprint-bound secondary-index names,
  tagged-payload expressions, concurrent lifecycle, owner verification, catalog
  readiness checks, and operation fencing;
- fail-closed partition admission, exact evidence identity, explicit regression
  limits, deterministic tenant-hash shadow names, and no destructive cutover SQL;
- exact-byte retained review and admitted archive verification bound to stable file
  and directory identities, metadata fingerprints, and recursive inventory without
  production lifecycle authorization;
- deterministic typed plans, controlled ordered binds, duplicate-free many-link
  `EXISTS` filtering, row-preserving nested many-link JSONB aggregation, strict
  decoded column contracts, exact count, lookahead, and scoped cursor construction;
- PostgreSQL-only query execution with exact persisted schema preflight, one
  read-only repeatable-read page/count snapshot, exhaustive bind conversion, and
  compiler-metadata-driven row mapping;
- deterministic source ownership, atomic cross-source registry materialization,
  no false empty runtime, one Index-owned PostgreSQL runtime constructor, and neutral
  capability transfer into `HostRuntimeContext`.

## M2 benchmark

M2 compared JSONB, normalized typed EAV, and specialized hot projection with one
deterministic Product/Variant/SalesChannel dataset and identical read, mutation,
and maintenance workloads. The exact successful packets and comparison are
archived under `docs/evidence/2026-07-27-postgresql-storage/`.

The accepted ADR selects JSONB. The typed-EAV and hot-projection implementations
are removed; `ops/benches` retains only a JSONB selected-layout regression harness.
It verifies source/JSONB result parity, transactional mutation evidence, committed
churn, relation size, WAL, buffers, and ordinary `VACUUM (ANALYZE)` behavior. Its
DDL remains benchmark-only and must not be copied into production migrations.

## Docs

- [Module documentation](./docs/README.md)
- [Live implementation plan](./docs/implementation-plan.md)
- [M4 source-owned schema registry](./docs/m4-source-schema-registry.md)
- [M4 query runtime composition](./docs/m4-query-runtime-composition.md)
- [M4 PostgreSQL query port contract](./docs/m4-postgres-query-port.md)
- [M4 many-link projection contract](./docs/m4-many-link-projection.md)
- [M2 storage benchmark contract](./docs/storage-benchmark.md)
- [M2 replacement evidence runbook](./docs/storage-evidence-runbook.md)
- [M3 retained partition capture runbook](./docs/partition-full-capture.md)
- [Index Engine rewrite ADR](../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Accepted storage ADR](../../DECISIONS/2026-07-24-index-storage-layout.md)
- [Platform docs index](../../docs/index.md)
