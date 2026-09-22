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
neutral shared query runtime. M5/M6 own the neutral replay-source catalog, bounded scan/load
contracts, one-page and bounded multi-page replay orchestration, durable schema-scoped replay
job ownership, lease-fenced checkpoints, cancellation, and host-published replay capability.
Source-specific Product, Content, Flex, search, scheduler, and long-running worker
implementations remain outside the engine core.

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
- `IndexSource`, `IndexSourceCatalog`, `IndexSourceDescriptor`, `SharedIndexSourceRegistry`
- `IndexSourceCursor`, `IndexSourceScanRequest`, `IndexSourcePage`
- `IndexSourceLoadRequest`, `IndexSourceLoadBatch`
- `IndexSourceFailure`, `IndexSourceFailureKind`, `IndexSourceError`
- `register_index_source`, `materialize_index_source_registry`
- `IndexReplayWorker`, `IndexReplayPageRequest`, `IndexReplayPageOutcome`,
  `IndexReplayPageStatus`
- `IndexReplayCheckpointKey`, `IndexReplayCheckpoint`, `IndexReplayCheckpointStore`
- `IndexReplayMutationSink`, `IndexReplayMutationOutcome`
- `IndexReplayFailure`, `IndexReplayFailureKind`, `IndexReplayError`
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
- `PostgresIndexReplayJobStore`, `IndexReplayJobLeaseRequest`, `IndexReplayJobLease`
- `IndexReplayJobAcquireOutcome`, `IndexReplayJobError`
- `PostgresIndexReplayCheckpointStore`
- `PostgresIndexReplayRunner`, `IndexReplayRunRequest`, `IndexReplayRunOutcome`
- `IndexReplayRunStatus`, `IndexReplayCancelOutcome`, `IndexReplayTerminalState`
- `IndexReplayRunError`
- `SharedIndexReplayRuntime`
- `materialize_postgres_index_replay_runtime`
- `IndexReplayRuntimeCompositionError`
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

`IndexReplayOperatorRuntime`, `IndexReplayOperatorContext`, and
`IndexReplayOperatorError` are server-owned guarded composition types. They are intentionally
not part of the engine crate API.

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

Source modules publish generic schema contracts through `IndexSchemaSourceCatalog`. The
catalog fixes one owner for every exact schema reference and for the complete schema identity
across versions. `rustok-distribution` materializes all contributions through one atomic
`SchemaRegistry::register_batch` and publishes `SharedIndexSchemaRegistry` only for a non-empty
valid catalog. Social Graph is the first source owner.

The server composes `SharedIndexQueryRuntime` through the Index-owned
`materialize_postgres_index_query_runtime`. The materializer binds the exact immutable shared
registry to the host `DatabaseConnection`, refuses duplicate runtime publication, performs no
SQL, and transfers the neutral capability through `ModuleRuntimeExtensions` and
`HostRuntimeContext`. Runtime presence does not claim PostgreSQL backend support for a test
connection, persisted tenant schema readiness, authorization, or successful query execution.

M5/M6 replay source, execution, ownership, and cancellation are source complete through the
bounded host-composition boundary. `IndexSourceCatalog` fixes one replay source for every exact
schema and complete schema identity across versions. `materialize_index_source_registry`
requires every source schema to be owner-published with the same owner and returns no false
runtime for an absent or empty source catalog. Cursor scans, targeted loads, cursor bytes,
page/key counts, tenant/schema scope, returned keys, and continuation progress are bounded.

`IndexReplayWorker::run_next_page` reads one checkpoint, scans one page, validates every
non-nil unique event UUID before the first write, applies mutations sequentially through an
`IndexReplayMutationSink`, and commits the next checkpoint only after all mutation results are
durable. Stable event UUIDs are delivery identities, so a checkpoint failure safely repeats the
old page through inbox deduplication. Checkpoint watermarks remain monotonic and completion is a
JSON null cursor.

`PostgresIndexReplayJobStore` owns one schema-scoped `rebuild` job for an exact
tenant/source/schema. It validates the `index_replay_job_v1` request and active persisted schema,
serializes acquisition with an advisory lock, heartbeats an unexpired lease, reclaims expired
work with an incremented attempt fence, and rejects stale heartbeat or terminal updates.
`PostgresIndexReplayCheckpointStore` requires an acquired `IndexReplayJobLease`; both reads and
writes lock and validate the exact `(job_id, worker_id, attempt_count)` before accessing the
checkpoint. Terminal success requires the same active lease and an exact durable null-cursor
checkpoint.

`PostgresIndexReplayRunner` resolves the source only from `SharedIndexSourceRegistry`, executes
at most 1024 pages per invocation, heartbeats between pages, yields unfinished work to an
immediately claimable pending attempt, and observes durable cancellation before and after page
work. Cancellation committed first cannot be overwritten by success, failure, or yield.

The server freezes `SharedIndexSourceRegistry` after module registration, calls
`materialize_postgres_index_replay_runtime`, and publishes `SharedIndexReplayRuntime` only when
both immutable schema and source registries exist. It then publishes the guarded
`IndexReplayOperatorRuntime`, which requires an exact request-bound tenant/actor permission
snapshot and `modules:manage` before delegating bounded run or cancellation. Transport adapters
must not call the raw shared replay runtime directly.

Runtime composition performs no SQL and starts no task. Runtime presence does not claim tenant
schema readiness, source availability, scheduler ownership, graceful shutdown, command
transport authorization, successful replay, or retained PostgreSQL evidence.

Still open are in-page interruption/timeouts, authorized GraphQL/HTTP/CLI command surfaces,
automatic retry/backoff and dead-letter state, host scheduling and task shutdown ownership,
locale/partition replay dimensions, production source adapters, reconciliation/drift repair,
and retained multi-instance PostgreSQL evidence.

Exact Decimal tagged-order transport is source-complete. Aggregate cursor continuation,
retained PostgreSQL/reference aggregate evidence, additional source schemas, transport
authorization, and first storefront/admin/search authoritative consumer cutover remain open.

No compatibility contract exists for deleted behavior. `IndexDocument`, `DocumentType`, old
ports/adapters, source DTOs/indexers/models/migrations, `IndexerRuntimeConfig`,
`IndexerContext`, and the old scheduler must not return.

## Dependencies on Other RusToK Crates

The generic engine core does not depend on source-domain crates. `rustok-core` supplies module
metadata and `ModuleRuntimeExtensions`. Source adapters remain in owner modules and publish
only generic Index contracts. Distribution and server composition must not import owner schema
builders or DTOs.

## Minimum Contract Set

### Input DTOs/Commands

- Input DTOs and query types are defined by `IndexQuery`, `IndexMutation`, `IndexRecord`, and related request types.
- Changes to the public fields of these types are breaking changes for index engine consumers.

### Domain Invariants

- Multi-tenant isolation, monotonic schema versions, and keyset ordering remain mandatory invariants.
- Missing tenant schemas, unauthorized cross-tenant operations, and stale fingerprints must fail closed.

### Events / Outbox Side Effects

- Mutation and replay events are processed through canonical sources and mutation stores.
- Index engine operations do not emit ad-hoc unversioned outbox events.

### Errors / Failure Codes

- `DomainError`, `SchemaRegistryError`, `CursorValidationError`, and `IndexSourceError` define the stable failure contracts of the crate.

### Schema source and runtime composition

- `IndexModule::register_runtime_extensions` seeds `IndexSchemaSourceCatalog` and
  `IndexSourceCatalog`.
- `register_index_schema_source` accepts one owner slug and one validated generic schema.
- Duplicate exact schema references fail even when fingerprints match.
- Different owners cannot split versions of one `(module, entity)` schema identity.
- `materialize_index_schema_registry` returns `None` for an absent or empty catalog.
- Non-empty schema materialization uses one atomic registration batch so cross-source links
  validate without partial registry state.
- `SharedIndexSchemaRegistry` wraps the exact immutable `Arc<SchemaRegistry>`; its constructor
  is not public.
- `register_index_source` accepts one owner, one bounded source name, exact schema references,
  and one neutral `IndexSource` implementation.
- One exact schema and complete schema identity cannot move between replay sources across
  versions.
- Replay-source materialization rejects unpublished schemas and schema/source owner drift.
- `materialize_index_source_registry` returns `None` for an absent or empty source catalog.
- `SharedIndexQueryRuntime` exposes only the transport-neutral `IndexQueryPort` capability.
- `materialize_postgres_index_query_runtime` is the production constructor for the PostgreSQL
  query runtime and fails when the runtime is already present.
- `SharedIndexReplayRuntime` exposes only bounded `run` and `request_cancel` operations.
- `materialize_postgres_index_replay_runtime` requires the immutable source registry and fails
  if the schema registry is missing or the replay runtime already exists.
- The server publishes `IndexReplayOperatorRuntime` after the raw replay runtime and requires
  an exact request-bound tenant/actor permission snapshot plus `modules:manage`.
- Runtime composition performs no SQL and starts no task.
- Executable hosts transfer capabilities through the existing typed runtime-extension seam.

### Replay source, job, and checkpoint

- `IndexSourceCursor` rejects JSON null and encoded values above 8 KiB, including during
  deserialization.
- `IndexSourceScanRequest` carries one non-nil tenant, one exact schema, one optional opaque
  cursor, and a limit from 1 through 1000.
- Scan pages cannot exceed the request, escape tenant/schema scope, repeat entity keys, return an
  empty continuation page, or return the same continuation cursor.
- `IndexSourceLoadRequest` carries one to 256 unique keys from one tenant and exact schema.
- Targeted load results cannot exceed the request, return another key, or duplicate a key.
- `IndexReplayWorker::run_next_page` validates all page event UUIDs before the first mutation
  write, applies mutations sequentially, and commits the checkpoint last.
- A replay source must return the same non-nil event UUID for the same logical mutation and source
  version when a page is retried.
- `IndexReplayJobLeaseRequest` validates tenant, source name, worker ID, schema, and whole-second
  lease duration from 1 through 86400 seconds.
- Job requests use the exact `index_replay_job_v1` JSON contract and require an active persisted
  schema.
- A non-expired owner returns `Busy`; an expired running attempt is reclaimed with an incremented
  attempt count; a succeeded job returns `AlreadyComplete`.
- Heartbeat and terminal updates require the exact job, worker, attempt, running state, and
  unexpired lease.
- `PostgresIndexReplayCheckpointStore::new` requires the acquired replay job lease.
- Checkpoint key tenant/source/schema must match the lease, and the exact active job attempt is
  row-locked before checkpoint read or write.
- A stale attempt cannot advance a checkpoint after reclaim.
- Successful job completion requires the active attempt and the exact rebuild checkpoint with a
  JSON null cursor.
- Bounded runs derive source ownership from the materialized registry, never caller input.
- A run processes at most its validated page budget and heartbeats only between pages.
- A cancellation request committed first wins over success, failure, and pending yield.
- The guarded server runtime rejects cross-tenant invocation and missing request-bound authority
  before delegating to the Index runtime.

### Query input and execution

- `IndexQueryScope` carries tenant and locale independently from caller filters.
- Selected, filtered, and ordered fields resolve through registered typed paths.
- Query shape, depth, selected fields, ordering expressions, page size, and offset are bounded.
- Plain `asc` / `desc` through a `many` path remains ambiguous and rejected.
- Explicit `min_asc`, `min_desc`, `max_asc`, and `max_desc` are accepted only for sortable
  scalar integer, Decimal, string, or timestamp fields reached through at least one `many` link
  and only with bounded offset pagination.
- Decimal aggregation and `ORDER BY` use PostgreSQL `numeric`; the hidden tagged order value uses
  a JSON string derived from `numeric::text`, matching the exact `IndexValue::Decimal` Serde wire
  without JSON-number or float conversion.
- Empty or all-null aggregate relation sets produce a nullable derived order value; ascending
  uses `NULLS LAST`, descending uses `NULLS FIRST`, and root entity ID remains the final tie-break.
- Boolean, UUID, list-valued, singular-path, cursor-paginated, and unsortable aggregate orders
  fail closed.
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
- `SchemaApplicationLeaseRequest`, `SecondaryIndexRequest`, and `IndexReplayJobLeaseRequest` use
  exact worker/attempt fencing.
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
- Replay sources cannot escape the requested tenant/schema or return unbounded pages/loads.
- Replay event IDs are non-nil and unique inside one page and remain stable across retry.
- Durable replay checkpoint progression is ordered after mutation outcomes and fenced by the
  active replay job attempt.
- A guarded replay invocation cannot widen its request-bound tenant scope.
- Generic engine types remain source-domain agnostic.
- Runtime composition is not a persisted-readiness assertion, scheduler, or evidence claim.

## Query Planning, Compilation, and Decoding

- `SchemaRegistry::plan_query` validates first, assigns stable aliases, resolves joins, propagates
  `traverses_many`, captures typed fields, groups many projections, and emits a v4 fingerprint.
- Aggregate-aware validation preserves legacy planner error variants for ordinary queries and
  marks derived many-order values nullable without changing the `PlannedOrder` shape.
- Many-traversing filters compile as independent nested correlated `EXISTS` chains.
- Many projections compile as correlated JSONB aggregates outside the outer root rowset and use
  stored link ordinal, entity identity, and locale for deterministic item order.
- Explicit many ordering compiles as a correlated typed `MIN` / `MAX` scalar subquery outside
  the outer root rowset; the selected order column remains tagged `IndexValue` JSONB for integer,
  Decimal, string, and timestamp wire types.
- Decimal hidden order JSON uses an exact string while its aggregate and ordering expressions
  remain typed `numeric`.
- `decode_postgres_query_page` re-plans and verifies the plan fingerprint, scalar/many metadata,
  tagged values, nested identity/value arity, uniqueness, page bounds, and optional exact count.
- Cursor pages remove lookahead and produce a next scoped cursor from the last retained
  entity/order tuple for ordinary ordering; aggregate cursor pages remain rejected.
- Offset pages report `has_more` without a cursor.

## Errors / Failure Codes

- `DomainError` defines identifier, schema-shape, and query-shape failures.
- `SchemaRegistryError` defines atomic registration and graph failures.
- `IndexSchemaSourceError` defines invalid schema-owner identity, duplicate exact ownership,
  owner drift across schema versions, empty materialization, invalid schema, and registry failures.
- `IndexSourceError` defines replay-source ownership conflicts, cursor/page/load bounds, result
  scope drift, continuation failures, and bounded source failures.
- `IndexReplayError` defines checkpoint identity, page event identity, mutation, and checkpoint
  progression failures.
- `IndexReplayJobError` defines job request, schema readiness, stored job, checkpoint completion,
  lease loss, and storage failures.
- `IndexReplayRunError` defines run bounds, cancellation identity, page failure, lease loss, and
  replay job failures.
- `IndexReplayRuntimeCompositionError` rejects duplicate runtime publication and a source runtime
  without the immutable schema registry.
- `IndexQueryRuntimeCompositionError` rejects duplicate shared query runtime materialization.
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
- Storage, lease, replay, secondary-index, and partition errors retain their typed ownership and
  evidence boundaries; transport adapters must not expose raw database details.

## Common AI Mistakes

- Adding Product, Content, Flex, Pricing, Inventory, or other source fields to engine enums.
- Reading source-module tables from Index or writing Index tables from source modules.
- Letting source modules own `index_jobs` or `index_checkpoints` writes.
- Creating an unfenced `PostgresIndexReplayCheckpointStore` without an acquired job lease.
- Advancing a checkpoint before every page mutation result is durable.
- Generating a new event UUID when replaying the same logical mutation and source version.
- Treating a bounded replay runtime as a completed scheduler, retry policy, or background worker.
- Treating in-memory registry composition as tenant-scoped persisted schema readiness.
- Constructing an ad hoc `SchemaRegistry` or `SharedIndexSchemaRegistry` in server/consumer code.
- Calling `PostgresIndexQueryPort::new` outside the Index-owned runtime materializer.
- Calling `PostgresIndexReplayRunner::new` outside the Index-owned replay runtime materializer.
- Letting GraphQL, HTTP, CLI, or admin transports call `SharedIndexReplayRuntime` instead of the
  server-owned guarded `IndexReplayOperatorRuntime`.
- Treating `SharedIndexQueryRuntime` presence as authorization or proof that a tenant query works.
- Publishing a consumer query without owner/transport authorization and bounded error mapping.
- Using plain `asc` / `desc`, link ordinal, first related row, or caller SQL as a many-order policy.
- Encoding Decimal aggregate order values as JSON numbers or floats instead of exact strings.
- Treating a source-complete aggregate compiler as PostgreSQL/reference execution evidence.
- Executing compiler SQL outside `PostgresIndexQueryPort` or splitting page/count snapshots.
