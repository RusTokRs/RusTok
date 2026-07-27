# rustok-index / CRATE_API

## Public Modules

- `domain`
- `application`
- `infrastructure`
- `migrations`

The domain/application contract remains database independent. M3 adds module-owned
production migrations, an Index-owned PostgreSQL mutation adapter, durable
schema-application leases, schema-derived secondary-index lifecycle, and a
fail-closed measured partition-admission contract; source-specific Content,
Product, Flex, search, legacy migration, runtime, and scheduler modules remain
deleted.

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

### Infrastructure

- `PostgresMutationStore`
- `MutationDelivery`
- `MutationApplyOutcome`
- `MutationStorageError`
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

The accepted M2 ADR selects JSONB, and M3 now registers the canonical schema for
`index_schemas`, `index_entities`, `index_links`, `index_inbox`, `index_jobs`,
`index_checkpoints`, and `index_consistency_findings`. `PostgresMutationStore`
atomically applies validated entity/link upserts and deletes through the durable
inbox. `PostgresSchemaLeaseStore` serializes exact tenant/schema application,
reclaims expired jobs with attempt fencing, and requires current ownership for
heartbeat and terminal completion. `SecondaryIndexPlan` derives stable indexes
from filterable/sortable schema fields, and `PostgresSecondaryIndexManager`
coordinates concurrent ensure/reindex/retire execution through durable fenced
jobs and PostgreSQL catalog readiness checks. Partition admission now requires an
exact retained evidence identifier, complete query/mutation/maintenance/cutover
measurement coverage, and an explicit policy before producing deterministic
tenant-hash shadow relation names and bootstrap SQL. It does not execute copy,
constraint/index attachment, dual-write/replay, cutover, or rollback. Source
registries, batch ingestion, rebuild, query-port, partition cutover, and operator
APIs remain later work.

No compatibility contract exists for deleted behavior. `IndexDocument`,
`DocumentType`, old ports/adapters, source DTOs/indexers/models/migrations,
`IndexerRuntimeConfig`, `IndexerContext`, and the old scheduler must not return.

## Dependencies on Other RusToK Crates

The generic engine core does not depend on source-domain crates. `rustok-core`
is used only for module metadata and platform contracts. Source adapters belong
to owner modules or explicit integration crates.

## Common AI Mistakes

- Adding Product, Content, Flex, Pricing, or Inventory fields to engine-core
  enums or structs.
- Reading source-module tables from Index.
- Treating Index as a ranking/full-text search engine.
- Reintroducing a catch-all JSON document as the public contract.
- Implementing rebuild by collecting every source ID before processing.
- Publishing unvalidated JSON filters instead of the typed query AST.
- Accepting a cursor without checking tenant, schema, fingerprint, locale, and
  order arity.
- Sorting through a `many` link without an explicit aggregate policy.
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
  the same version is rejected by `SchemaRegistry`.

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

- Registration is atomic for a batch.
- Re-registering an identical schema version is idempotent.
- Changing a contract under the same version is an error.
- Versions for a schema identity are monotonic.
- Link paths resolve deterministically through the registered graph.
- Schema fingerprints ignore declaration order but include all semantic field,
  link, locale, and version metadata.

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

### Cursor Contract

- Cursor format is explicitly versioned.
- Payload uses postcard and URL-safe Base64.
- A checksum detects corruption.
- Cursor application validates tenant, schema, schema fingerprint, locale,
  ordering arity, and entity tie-breaker identity.
- Cursor integrity is not an authorization substitute; transport and query
  policy still enforce caller access.

### Events / Outbox Side Effects

- Source events are converted to `IndexMutation` through owner-published
  adapters.
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
- `SchemaRegistryError` defines registration and graph failures.
- `RecordValidationError` and `QueryValidationError` define registry-backed data
  and query failures.
- `CursorCodecError` and `CursorValidationError` separate malformed cursors from
  scope/schema mismatches.
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
- Later milestones add source, retry, cancellation, and rebuild errors.
