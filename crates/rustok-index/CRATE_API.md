# rustok-index / CRATE_API

## Public Modules

- `domain`
- `application`
- `infrastructure`
- `migrations`

The domain/application contract remains database independent. M3 adds module-owned
production migrations, an Index-owned PostgreSQL mutation adapter, tenant-scoped
source schema persistence, durable schema-application leases, schema-derived
secondary-index lifecycle, and a fail-closed measured partition-admission contract;
source-specific Content, Product, Flex, search, legacy migration, runtime, and
scheduler modules remain deleted.

## Primary Public Types

### Domain

- `IndexModule`
- `ModuleName`, `SchemaIdentity`, `SchemaRef`, `SchemaVersion`
- `EntityName`, `EntityKey`, `FieldName`, `FieldPath`, `LinkName`, `LocaleKey`
- `IndexValue`, `IndexValueType`
- `IndexSchema`, `IndexField`, `IndexLink`, `SchemaFingerprint`
- `IndexRecord`, `IndexLinkValue`, `LinkedEntityKey`
- `IndexMutation`
- `IndexQueryScope`, `IndexQuery`, `FilterExpr`, `OrderExpr`,
  `OrderDirection`, `Pagination`
- `DomainError`

### Application

- `SchemaRegistry`, `RegisteredSchema`, `RegistrationOutcome`
- `SchemaRegistryError`, `LinkPathStep`
- `RecordValidationError`, `QueryValidationError`
- `IndexCursor`, `CursorCodec`, `CursorCodecError`, `CursorValidationError`
- `ExecutableQueryPlan`, `PlannedJoin`, `PlannedField`, `PlannedOrder`
- `QueryPlanFingerprint`, `QueryPlanError`
- `PostgresBindValue`, `CompiledQueryColumn`, `CompiledPostgresCount`
- `CompiledPostgresQuery`, `PostgresQueryBuildError`, `PostgresQueryCompileError`
- `CompiledPostgresCell`, `CompiledPostgresRow`, `CompiledPostgresPageQuery`
- `IndexProjectedValue`, `IndexRelationIdentity`, `IndexQueryItem`, `IndexQueryPage`
- `PostgresQueryPageBuildError`, `PostgresQueryDecodeError`

### Infrastructure

- `PostgresMutationStore`
- `MutationDelivery`
- `MutationApplyOutcome`
- `MutationStorageError`
- `PostgresSchemaRegistrationStore`
- `PersistedSchemaRegistrationOutcome`
- `SchemaRegistrationError`
- `PostgresSchemaLeaseStore`
- `SchemaApplicationLeaseRequest`
- `SchemaApplicationLease`
- `SchemaLeaseAcquireOutcome`
- `SchemaLeaseError`
- `SecondaryIndexPlan`
- `SecondaryIndexSpec`
- `SecondaryIndexKind`
- `SecondaryIndexOperation`
- `SecondaryIndexRequest`
- `SecondaryIndexLease`
- `SecondaryIndexClaimOutcome`
- `SecondaryIndexExecutionOutcome`
- `SecondaryIndexError`
- `PostgresSecondaryIndexManager`
- `PartitionStrategy`
- `PartitionAdmissionPolicy`
- `PartitionBaselineEvidence`
- `PartitionMeasurementCoverage`
- `PartitionShadowEvidence`
- `PartitionEvidence`
- `PartitionAdmissionReason`
- `PartitionAdmissionOutcome`
- `PartitionRelationPlan`
- `PartitionShadowPlan`
- `PartitionAdmissionError`
- `evaluate_partition_admission`

## Contract Status

M1 domain/application contracts are active. They provide canonical identifiers
and locales, stable schema fingerprints, atomic schema registration,
deterministic link paths, record/query validation, bounded query complexity,
and query-scoped keyset cursors.

The accepted M2 ADR selects JSONB, and M3 registers the canonical schema for
`index_schemas`, `index_entities`, `index_links`, `index_inbox`, `index_jobs`,
`index_checkpoints`, and `index_consistency_findings`.
`PostgresSchemaRegistrationStore` is the generic Index-owned boundary for
source-published tenant schemas. It validates the domain contract, serializes one
tenant/schema identity under a PostgreSQL transaction advisory lock, inserts an
active schema idempotently, rejects same-version contract reuse, lower-version
insertion, retired reactivation, nil tenants, and unsupported backends. Source
owners do not write `index_schemas` directly.

`PostgresMutationStore` atomically applies validated entity/link upserts and
deletes through the durable inbox. `PostgresSchemaLeaseStore` serializes exact
tenant/schema application, reclaims expired jobs with attempt fencing, and
requires current ownership for heartbeat and terminal completion.
`SecondaryIndexPlan` derives stable indexes from filterable/sortable schema fields,
and `PostgresSecondaryIndexManager` coordinates concurrent ensure/reindex/retire
execution through durable fenced jobs and PostgreSQL catalog readiness checks.
Partition admission requires an exact retained evidence identifier, complete
query/mutation/maintenance/cutover measurement coverage, and an explicit policy
before producing deterministic tenant-hash shadow relation names and bootstrap
SQL. It does not execute copy, constraint/index attachment, dual-write/replay,
cutover, or rollback.

M4 now provides validated typed executable plans, controlled PostgreSQL SQL and bind
DTOs for root and explicit one-cardinality-link query semantics, query-scoped cursor
continuation, one-row page lookahead, strict compiled-column/result decoding,
exact-count decoding, `has_more`, and next-cursor construction. It still does not
execute statements, adapt SeaORM rows directly, implement many-link semantics,
publish `IndexQueryPort`, or claim PostgreSQL/reference-engine equivalence.
Multi-source catalog composition, batch ingestion, rebuild, query-port, partition
cutover, and operator APIs remain later work.

No compatibility contract exists for deleted behavior. `IndexDocument`,
`DocumentType`, old ports/adapters, source DTOs/indexers/models/migrations,
`IndexerRuntimeConfig`, `IndexerContext`, and the old scheduler must not return.

## Dependencies on Other RusToK Crates

The generic engine core does not depend on source-domain crates. `rustok-core`
is used only for module metadata and platform contracts. Source adapters belong
to owner modules or explicit integration crates and register through Index-owned
APIs.

## Common AI Mistakes

- Adding Product, Content, Flex, Pricing, or Inventory fields to engine-core
  enums or structs.
- Reading source-module tables from Index.
- Writing `index_schemas` directly from a source module instead of calling
  `PostgresSchemaRegistrationStore`.
- Treating in-memory `SchemaRegistry` registration as persisted tenant schema
  readiness for `PostgresMutationStore`.
- Reactivating a retired schema or silently replacing a contract under the same
  schema version.
- Treating Index as a ranking/full-text search engine.
- Reintroducing a catch-all JSON document as the public contract.
- Implementing rebuild by collecting every source ID before processing.
- Publishing unvalidated JSON filters instead of the typed query AST.
- Accepting a cursor without checking tenant, schema, fingerprint, locale, filter,
  ordered fields/directions, order arity, and order-value types.
- Executing a page query through raw `compile_postgres_query` instead of the
  one-row-lookahead `compile_postgres_page_query` handoff.
- Decoding rows without rechecking the plan fingerprint and exact compiled column
  contract.
- Sorting or projecting through a `many` link without an explicit aggregate policy.
- Completing or heartbeating schema/index work without exact worker and attempt
  fencing.
- Building expression indexes against `payload ->> field`; stored `IndexValue`
  payloads are tagged and scalar/list values live under each field's `value` key.
- Creating bespoke Product or other owner-specific indexes instead of deriving
  them from the generic schema contract.
- Enabling partitioning because a relation is merely large, without retained
  tenant-scoped baseline and shadow evidence.
- Treating zero measured runs or less than 100% tenant-predicate coverage as
  acceptable partition evidence.
- Treating `PartitionShadowPlan::bootstrap_statements` as cutover-ready DDL; it
  intentionally contains no production rename/drop or constraint/index attachment.
- Restoring deleted v1 or source-specific code as a compatibility layer.

## Minimum Contract Set

### Input DTOs/Commands

- `IndexSchema`, `IndexRecord`, `IndexMutation`, and `IndexQuery` are the current
  input contracts.
- `IndexQueryScope` carries tenant and locale independently from caller filters.
- `SchemaRegistry::compile_postgres_page_query` is the page-execution compiler
  handoff; it preserves SQL and increases only the validated limit bind by one.
- `CompiledPostgresRow` is the narrow adapter handoff for compiler-owned UUID,
  tagged JSON, SQL-null, and exact-count cells.
- `PostgresSchemaRegistrationStore::register(tenant_id, schema)` binds one non-nil
  tenant to one validated exact schema contract and calculated fingerprint.
- `SchemaApplicationLeaseRequest` binds one tenant, exact schema reference,
  computed fingerprint, worker identity, and bounded whole-second lease duration.
- `SecondaryIndexPlan` binds one tenant and exact schema fingerprint to all
  filterable/sortable field indexes.
- `SecondaryIndexRequest` binds one immutable index spec, operation, worker, and
  bounded whole-second lease duration.
- `PartitionEvidence` binds measured unpartitioned baseline facts to one exact
  SHA-256 identified tenant-hash shadow packet.
- `PartitionMeasurementCoverage` records non-zero query, mutation, maintenance,
  and cutover-rehearsal run counts. A missing group is a typed rejection reason.
- `PartitionAdmissionPolicy` supplies explicit minimum scale and maximum
  regression/skew/lock thresholds; no production default silently admits rollout,
  and tenant-predicate coverage is fixed at 10000 basis points.
- Construction and validation preserve tenant, schema, entity, locale, and
  source-version identity.
- Identifiers use bounded lowercase ASCII grammar; locales use ICU4X
  canonicalization.
- Public field changes require a new `SchemaVersion`; incompatible content under
  the same version is rejected by both in-memory and persisted registration.

### Domain Invariants

- Every record and query is explicitly tenant scoped.
- Locale presence follows the registered schema's `LocaleMode`.
- Every record belongs to an exact registered schema version.
- Record values match field type, nullability, and cardinality.
- Link targets match registered target schemas, fields, join types, locale mode,
  and cardinality.
- Selected, filtered, and ordered fields are resolved through typed link paths.
- Query complexity, path depth, page size, and offset depth are bounded.
- Sorting through a `many` link is rejected until aggregation is explicit.
- Source versions and tombstones prevent stale mutation overwrite.
- Generic engine types remain source-domain agnostic.

### Schema Registry

- In-memory registration is atomic for a batch.
- Re-registering an identical schema version is idempotent.
- Changing a contract under the same version is an error.
- Versions for a schema identity are monotonic.
- Link paths resolve deterministically through the registered graph.
- Schema fingerprints ignore declaration order but include all semantic field,
  link, locale, and version metadata.

### Persisted Source Schema Registration

- Registration is tenant scoped and supports PostgreSQL and SQLite only.
- PostgreSQL serializes the tenant/module/entity identity with a transaction-scoped
  advisory lock before exact/latest-version checks.
- An exact active schema with matching fingerprint and semantic JSON is
  `Unchanged`; a new greater version is `Inserted`.
- Same-version contract drift, an unregistered lower version, retired state, nil
  tenant, malformed persisted values, and storage failures fail closed.
- Registration commits before an owner mutation may rely on the schema foreign key.
- The store contains no source-domain types or table reads outside Index-owned
  storage.

### Schema Application Lease

- Acquisition is scoped by tenant, module, entity, and schema version.
- PostgreSQL acquisition takes a transaction-scoped advisory lock before reading
  persisted schema and job state.
- The persisted schema must be active and match the request fingerprint.
- A non-expired running owner returns `Busy`; a succeeded application returns
  `AlreadyApplied`.
- Expired work is reclaimed by increasing `attempt_count` on the same job.
- Heartbeat, success, and failure require the exact job, worker, attempt, running
  state, and an unexpired lease.
- Failed terminal jobs permit a new job; succeeded jobs remain terminal.

### Secondary Index Lifecycle

- Plans include only fields declared filterable or sortable by the exact schema.
- Scalar fields use deterministic typed partial B-tree expressions ordered by
  locale, value, and entity identity.
- Filterable `many` fields use field-local JSONB containment GIN.
- Expressions read the tagged production `IndexValue` payload through the field's
  `value` member; timestamp keys use an immutable canonical UTC-digit expression.
- Names bind tenant, schema reference, schema fingerprint, field type,
  cardinality, index kind, and payload contract through SHA-256.
- Ensure, reindex, and retire use `CREATE INDEX CONCURRENTLY`,
  `REINDEX INDEX CONCURRENTLY`, and `DROP INDEX CONCURRENTLY` in PostgreSQL.
- Active jobs are serialized by a transaction advisory lock and fenced by worker,
  attempt count, state, and lease expiry.
- PostgreSQL owner comments bind the index name to its full definition hash;
  conflicting ownership fails closed.
- Completion requires catalog `indisready` and `indisvalid`; retirement remains
  available after a schema is retired.

### Partition Admission and Shadow Planning

- The canonical `index_entities` and `index_links` tables remain unpartitioned by
  default because M2 did not measure partitioning.
- Tenant-hash modulus must be a power of two between 2 and 128.
- Evidence identity is a lowercase 64-character SHA-256 digest.
- Admission requires exactly 10000 basis points of tenant-predicate coverage.
- Query, mutation, maintenance, and cutover-rehearsal measurement counts must each
  be non-zero.
- Admission also checks measured total rows/bytes, distinct tenants, entity/link
  digest parity, catch-up, foreign keys, orphan links, query-plan regressions, p95
  query/mutation regressions, WAL amplification, partition-size skew, and
  cutover-lock duration.
- Any failed gate returns `KeepUnpartitioned` with typed reasons.
- An admitted plan derives deterministic shadow parent/partition names from the
  evidence identity, strategy, modulus, and plan-contract version.
- Bootstrap SQL creates only shadow hash-partition parents and children. It never
  renames, drops, or alters the production entity/link relations.
- Copy, constraints, indexes, replay/dual-write, cutover, rollback, durable global
  ownership, and PostgreSQL evidence remain mandatory future work.

### Query Planning, Compilation, and Result Decoding

- `SchemaRegistry::plan_query` validates first and captures deterministic aliases,
  joins, typed referenced fields, projection, filters, ordering, pagination, and a
  versioned plan fingerprint.
- `compile_postgres_query` accepts only the supported root/one-link subset and emits
  controlled SQL plus ordered bind DTOs; caller values and contract names remain
  binds.
- `compile_postgres_page_query` changes only the validated main-statement page-limit
  bind from `N` to `N + 1`; offset and exact-count binds are preserved.
- `decode_postgres_query_page` re-plans the query, compares the plan fingerprint and
  complete column metadata, validates every tagged field value, and rejects more
  than `N + 1` rows.
- The lookahead row is removed. Cursor pages produce `has_more` and a scoped next
  cursor from the last retained entity/order tuple; offset pages produce
  `has_more` without a cursor.
- SQL execution, SeaORM row adaptation, many-link semantics, and live equivalence
  evidence remain separate future boundaries.

### Cursor Contract

- Cursor formats are explicitly versioned.
- Payloads use postcard and URL-safe Base64.
- A checksum detects corruption.
- Production continuation tokens bind tenant, schema, locale, filter, ordered
  fields, and directions through a query fingerprint.
- Cursor application validates schema fingerprint, ordering arity, order-value
  types, and non-nil entity tie-breaker identity.
- Cursor integrity is not an authorization substitute; transport and query
  policy still enforce caller access.

### Events / Outbox Side Effects

- Source events are converted to `IndexMutation` through owner-published adapters.
- The source schema is persisted through the Index owner before a mutation relies
  on it.
- Delivery is replayable and idempotent.
- `MutationDelivery` binds a source name and delivery ID to one exact serialized
  `IndexMutation` payload.
- `PostgresMutationStore` claims the tenant/source/delivery inbox identity,
  rejects payload reuse, and commits the inbox terminal state with the entity/link
  mutation.
- Exact redelivery is `Duplicate`; stale source versions are terminally ignored;
  live upserts and tombstones replace links atomically.

### Errors / Failure Codes

- `DomainError` defines identifier, schema-shape, and query-shape failures.
- `SchemaRegistryError` defines in-memory registration and graph failures.
- `SchemaRegistrationError` separates nil tenant, invalid schema, same-version
  conflict, non-monotonic version, retired state, and generic storage failure.
- `RecordValidationError` and `QueryValidationError` define registry-backed data
  and query failures.
- `CursorCodecError` and `CursorValidationError` separate malformed cursors from
  scope/schema/query-fingerprint/type mismatches.
- `QueryPlanError`, `PostgresQueryBuildError`, and `PostgresQueryCompileError`
  separate validation/planning failures from unsupported or corrupted compiler
  contracts.
- `PostgresQueryPageBuildError` rejects missing or mismatched pagination binds;
  `PostgresQueryDecodeError` rejects plan/column/count mismatches, malformed cells,
  invalid tagged values, unexpected nulls, and oversized result batches.
- `MutationStorageError` separates validation, delivery identity conflict,
  in-progress/rejected replay, stored-version corruption, backend limits, and
  database failure. Its public display is generic; transport adapters must still
  map owner errors rather than returning storage details directly.
- `SchemaLeaseError` separates request validation, missing/retired/conflicting
  schema state, malformed durable jobs, lost ownership, and database failure.
- `SecondaryIndexError` separates plan/request validation, schema conflicts,
  malformed jobs, lease loss, ownership conflicts, missing/not-ready indexes,
  unsupported backends, and storage failures.
- `PartitionAdmissionError` separates invalid policy, invalid evidence, metric
  overflow, and unsupported hash modulus. Typed admission reasons explain every
  rejected evidence gate without exposing storage internals.
- Later milestones add source catalog, retry, cancellation, rebuild, and query-port
  execution errors.
