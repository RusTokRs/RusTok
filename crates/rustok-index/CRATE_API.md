# rustok-index / CRATE_API

## Public Modules

- `domain`
- `application`
- `infrastructure`
- `migrations`

The domain/application contract remains database independent. M3 owns the canonical
PostgreSQL storage, mutation, schema-registration, lease, secondary-index, and measured
partition-admission boundaries. M4 owns typed query planning, controlled SQL compilation,
strict decoding, PostgreSQL execution, source-owned schema catalog composition, and the
neutral shared query runtime. Source-specific Product, Content, Flex, search, migration,
and scheduler implementations remain outside the engine core.

## Primary Public Types

### Domain

- `IndexModule`
- `ModuleName`, `SchemaIdentity`, `SchemaRef`, `SchemaVersion`
- `EntityName`, `EntityKey`, `FieldName`, `FieldPath`, `LinkName`, `LocaleKey`
- `IndexValue`, `IndexValueType`
- `IndexSchema`, `IndexField`, `IndexLink`, `SchemaFingerprint`
- `IndexRecord`, `IndexLinkValue`, `LinkedEntityKey`
- `IndexMutation`
- `IndexQueryScope`, `IndexQuery`, `FilterExpr`, `OrderExpr`, `OrderDirection`,
  `ManyOrderAggregate`, `Pagination`
- `DomainError`

### Application

- `SchemaRegistry`, `RegisteredSchema`, `RegistrationOutcome`
- `SchemaRegistryError`, `LinkPathStep`
- `IndexSchemaSourceCatalog`, `IndexSchemaSourceDescriptor`, `IndexSchemaSourceError`
- `SharedIndexSchemaRegistry`
- `register_index_schema_source`, `materialize_index_schema_registry`
- `RecordValidationError`, `QueryValidationError`, `AggregateOrderValidationError`
- `IndexCursor`, `CursorCodec`, `CursorCodecError`, `CursorValidationError`
- `ExecutableQueryPlan`, `PlannedJoin`, `PlannedField`, `PlannedManyProjection`, `PlannedOrder`
- `QueryPlanFingerprint`, `QueryPlanError`
- `PostgresBindValue`, `CompiledQueryColumn`, `CompiledManyRelationColumn`, `CompiledPostgresCount`
- `CompiledPostgresQuery`, `PostgresQueryBuildError`, `PostgresQueryCompileError`
- `CompiledPostgresCell`, `CompiledPostgresRow`, `CompiledPostgresPageQuery`
- `IndexProjectedValue`, `IndexRelationIdentity`, `IndexNestedRelationItem`
- `IndexNestedRelationProjection`, `IndexQueryItem`, `IndexQueryPage`
- `PostgresQueryPageBuildError`, `PostgresQueryDecodeError`
- `IndexQueryPort`, `SharedIndexQueryRuntime`
- `IndexQueryExecutionError`, `PersistedSchemaReadinessFailure`

### Infrastructure

- `PostgresIndexQueryPort`
- `materialize_postgres_index_query_runtime`
- `IndexQueryRuntimeCompositionError`
- `PostgresMutationStore`, `MutationDelivery`, `MutationApplyOutcome`, `MutationStorageError`
- `PostgresSchemaRegistrationStore`, `PersistedSchemaRegistrationOutcome`, `SchemaRegistrationError`
- `PostgresSchemaLeaseStore`, `SchemaApplicationLeaseRequest`, `SchemaApplicationLease`
- `SchemaLeaseAcquireOutcome`, `SchemaLeaseError`
- `SecondaryIndexPlan`, `SecondaryIndexSpec`, `SecondaryIndexKind`, `SecondaryIndexOperation`
- `SecondaryIndexRequest`, `SecondaryIndexLease`, `SecondaryIndexClaimOutcome`
- `SecondaryIndexExecutionOutcome`, `SecondaryIndexError`, `PostgresSecondaryIndexManager`
- `PartitionStrategy`, `PartitionAdmissionPolicy`, `PartitionBaselineEvidence`
- `PartitionMeasurementCoverage`, `PartitionShadowEvidence`, `PartitionEvidence`
- `PartitionAdmissionReason`, `PartitionAdmissionOutcome`, `PartitionRelationPlan`
- `PartitionShadowPlan`, `PartitionAdmissionError`, `evaluate_partition_admission`

## Contract Status

M1 generic domain/application contracts are active. They provide canonical identifiers and
locales, stable schema fingerprints, atomic schema registration, deterministic link paths,
record/query validation, bounded query complexity, and query-scoped cursors.

M3 owns the canonical `index_schemas`, `index_entities`, `index_links`, `index_inbox`,
`index_jobs`, `index_checkpoints`, and `index_consistency_findings` schema. Source modules
persist tenant schema readiness only through `PostgresSchemaRegistrationStore`; they never
write Index tables directly. Mutation persistence, schema leases, secondary-index lifecycle,
and partition admission remain Index-owned and fail closed on identity or evidence drift.

M4 provides validated executable plans, controlled PostgreSQL SQL and ordered binds,
root/one-link projection and ordering, correlated many-link filtering, deterministic nested
many-link projection aggregates, explicit `min` / `max` many-link ordering for bounded offset
pages, exact count, query-scoped keyset pagination for ordinary order expressions, one-row
lookahead, strict row decoding, and PostgreSQL execution through one read-only repeatable-read
snapshot.

Source modules now publish generic schema contracts through `IndexSchemaSourceCatalog`.
The catalog fixes one owner for every exact schema reference and for the complete schema
identity across versions. `rustok-distribution` materializes all contributions through one
atomic `SchemaRegistry::register_batch` and publishes `SharedIndexSchemaRegistry` only for a
non-empty valid catalog. Social Graph is the first source owner.

The server composes `SharedIndexQueryRuntime` through the Index-owned
`materialize_postgres_index_query_runtime`. The materializer binds the exact immutable shared
registry to the host `DatabaseConnection`, refuses duplicate runtime publication, performs no
SQL, and transfers the neutral capability through `ModuleRuntimeExtensions` and
`HostRuntimeContext`. Runtime presence does not claim PostgreSQL backend support for a test
connection, persisted tenant schema readiness, authorization, or successful query execution.

Exact Decimal tagged-order transport, aggregate cursor continuation, retained
PostgreSQL/reference aggregate evidence, additional source schemas, transport authorization,
and first storefront/admin/search authoritative consumer cutover remain open.

No compatibility contract exists for deleted behavior. `IndexDocument`, `DocumentType`, old
ports/adapters, source DTOs/indexers/models/migrations, `IndexerRuntimeConfig`,
`IndexerContext`, and the old scheduler must not return.

## Dependencies on Other RusToK Crates

The generic engine core does not depend on source-domain crates. `rustok-core` supplies module
metadata and `ModuleRuntimeExtensions`. Source adapters remain in owner modules and publish
only generic Index contracts. Distribution and server composition must not import owner schema
builders or DTOs.

## Minimum Contract Set

### Schema source and runtime composition

- `IndexModule::register_runtime_extensions` seeds `IndexSchemaSourceCatalog`.
- `register_index_schema_source` accepts one owner slug and one validated generic schema.
- Duplicate exact references fail even when fingerprints match.
- Different owners cannot split versions of one `(module, entity)` identity.
- `materialize_index_schema_registry` returns `None` for an absent or empty catalog.
- Non-empty materialization uses one atomic registration batch so cross-source links validate
  without partial registry state.
- `SharedIndexSchemaRegistry` wraps the exact immutable `Arc<SchemaRegistry>`; its constructor
  is not public.
- `SharedIndexQueryRuntime` exposes only the transport-neutral `IndexQueryPort` capability.
- `materialize_postgres_index_query_runtime` is the production constructor for the PostgreSQL
  runtime and fails when the runtime is already present.
- Runtime composition performs no SQL and does not replace tenant-scoped persisted schema
  registration or preflight.
- Executable hosts transfer the capability through the existing typed runtime-extension seam.

### Query input and execution

- `IndexQueryScope` carries tenant and locale independently from caller filters.
- Selected, filtered, and ordered fields resolve through registered typed paths.
- Query shape, depth, selected fields, ordering expressions, page size, and offset are bounded.
- Plain `asc` / `desc` through a `many` path remains ambiguous and rejected.
- Explicit `min_asc`, `min_desc`, `max_asc`, and `max_desc` are accepted only for sortable
  scalar integer, string, or timestamp fields reached through at least one `many` link and only
  with bounded offset pagination.
- Empty or all-null aggregate relation sets produce a nullable derived order value; ascending
  uses `NULLS LAST`, descending uses `NULLS FIRST`, and root entity ID remains the final tie-break.
- Boolean, Decimal, UUID, list-valued, singular-path, cursor-paginated, and unsortable aggregate
  orders fail closed. Decimal requires a separate exact tagged-wire contract before enablement.
- `SchemaRegistry::compile_postgres_page_query` preserves the compiled query and changes only
  the validated page-limit bind from `N` to `N + 1`.
- `CompiledPostgresQuery::many_relations` binds every aggregate alias to exact plan metadata.
- `CompiledPostgresRow` contains only compiler-owned UUID, tagged JSON, nested aggregate JSON,
  SQL-null, and exact-count cells.
- `PostgresIndexQueryPort` owns one connection and one immutable registry. It accepts no raw SQL
  or caller-provided result metadata.
- Every execution verifies root/source/target schemas against the query tenant's exact active
  persisted fingerprint and semantic JSON contract.
- Page and optional exact-count statements execute in one read-only repeatable-read snapshot.
- Authentication and transport policy remain caller responsibilities and must not widen the
  tenant or locale scope already present in `IndexQuery`.

### Mutation and storage

- `PostgresSchemaRegistrationStore::register` binds one non-nil tenant to one exact validated
  schema and calculated fingerprint.
- `PostgresMutationStore` applies one `MutationDelivery` with inbox deduplication, complete
  entity-key serialization, monotonic source versions, tombstones, and atomic link replacement.
- `SchemaApplicationLeaseRequest` and `SecondaryIndexRequest` use exact worker/attempt fencing.
- Partition admission requires one exact retained evidence identity, non-zero coverage groups,
  100% tenant-predicate coverage, parity, catch-up, integrity, regression, WAL, skew, and lock
  gates before producing shadow-only planning output.
- Production partition copy, constraint/index attachment, replay/dual-write, cutover, rollback,
  and global operation ownership remain outside the current source-complete boundary.

## Domain Invariants

- Every record and query is tenant scoped.
- Locale presence follows the registered schema's `LocaleMode`.
- Records belong to one exact schema version and values match type, cardinality, and nullability.
- Link targets match registered target schemas, join fields, types, locale modes, and cardinality.
- Filtering through `many` paths uses correlated existential semantics and cannot duplicate root
  rows or exact counts.
- Projection through `many` paths returns deterministic nested items with complete identity
  chains and aligned tagged values.
- Ordering through `many` paths requires an explicit supported `min` / `max` mode; implicit
  first-row, link-ordinal, and storage-order semantics remain forbidden.
- Source versions and tombstones prevent stale mutation overwrite.
- Generic engine types remain source-domain agnostic.
- Runtime composition is not an authorization decision or persisted-readiness assertion.

## Query Planning, Compilation, and Decoding

- `SchemaRegistry::plan_query` validates first, assigns stable aliases, resolves joins, propagates
  `traverses_many`, captures typed fields, groups many projections, and emits a v4 fingerprint.
- Aggregate-aware validation preserves legacy planner error variants for ordinary queries and
  marks derived many-order values nullable without changing the `PlannedOrder` shape.
- Many-traversing filters compile as independent nested correlated `EXISTS` chains.
- Many projections compile as correlated JSONB aggregates outside the outer root rowset and use
  stored link ordinal, entity identity, and locale for deterministic item order.
- Explicit many ordering compiles as a correlated typed `MIN` / `MAX` scalar subquery outside
  the outer root rowset; the selected order column remains tagged `IndexValue` JSONB for the
  currently admitted integer, string, and timestamp wire types.
- `decode_postgres_query_page` re-plans and verifies the plan fingerprint, scalar/many metadata,
  tagged values, nested identity/value arity, uniqueness, page bounds, and optional exact count.
- Cursor pages remove lookahead and produce a next scoped cursor from the last retained
  entity/order tuple for ordinary ordering; aggregate cursor pages remain rejected.
- Offset pages report `has_more` without a cursor.

## Errors / Failure Codes

- `DomainError` defines identifier, schema-shape, and query-shape failures.
- `SchemaRegistryError` defines atomic registration and graph failures.
- `IndexSchemaSourceError` defines invalid owner identity, duplicate exact ownership, owner drift
  across schema versions, empty materialization, invalid schema, and registry failures.
- `IndexQueryRuntimeCompositionError` currently rejects duplicate shared runtime materialization.
- `RecordValidationError`, `QueryValidationError`, and `AggregateOrderValidationError` define
  registry-backed data/query failures and the bounded aggregate-order policy.
- `CursorCodecError` and `CursorValidationError` separate malformed cursors from scope, schema,
  fingerprint, query-shape, arity, and value-type mismatches.
- `QueryPlanError`, `PostgresQueryBuildError`, and `PostgresQueryCompileError` separate
  validation/planning from unsupported or corrupted compiler contracts.
- `PostgresQueryDecodeError` rejects plan/column/count mismatch, malformed tagged values, nested
  arity drift, nil/duplicate identities, unexpected nulls, and oversized batches.
- `IndexQueryExecutionError` separates unsupported backend, missing/inactive/drifted persisted
  schemas, build/decode failures, missing counts, invalid driver columns, contract preparation,
  and storage operations while keeping top-level display bounded.
- Storage, lease, secondary-index, and partition errors retain their typed ownership and evidence
  boundaries; transport adapters must not expose raw database details.

## Common AI Mistakes

- Adding Product, Content, Flex, Pricing, Inventory, or other source fields to engine enums.
- Reading source-module tables from Index or writing Index tables from source modules.
- Treating in-memory registry composition as tenant-scoped persisted schema readiness.
- Constructing an ad hoc `SchemaRegistry` or `SharedIndexSchemaRegistry` in server/consumer code.
- Calling `PostgresIndexQueryPort::new` outside the Index-owned runtime materializer.
- Treating `SharedIndexQueryRuntime` presence as authorization or proof that a tenant query works.
- Publishing a consumer query without owner/transport authorization and bounded error mapping.
- Using plain `asc` / `desc`, link ordinal, first related row, or caller SQL as a many-order policy.
- Enabling Decimal aggregate ordering before an exact tagged-order wire contract is source-complete.
- Treating a source-complete aggregate compiler as PostgreSQL/reference execution evidence.
- Executing compiler SQL outside `PostgresIndexQueryPort` or splitting page/count snapshots.
