# rustok-index / CRATE_API

## Public Modules

- `domain`
- `application`
- `infrastructure`
- `migrations`

The domain/application contract remains database independent. M3 adds module-owned
production migrations and an Index-owned PostgreSQL mutation adapter;
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

### Infrastructure

- `PostgresMutationStore`
- `MutationDelivery`
- `MutationApplyOutcome`
- `MutationStorageError`

## Contract Status

M1 domain/application contracts are active. They provide canonical identifiers
and locales, stable schema fingerprints, atomic schema registration,
deterministic link paths, record/query validation, bounded query complexity,
and query-scoped keyset cursors.

The accepted M2 ADR selects JSONB, and M3 now registers the canonical schema for
`index_schemas`, `index_entities`, `index_links`, `index_inbox`, `index_jobs`,
`index_checkpoints`, and `index_consistency_findings`. The first runtime contract is
`PostgresMutationStore`, which atomically applies validated entity/link upserts and
deletes through the durable inbox. Source registries, batch ingestion, rebuild,
query-port, and operator APIs are published by later milestones.

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
- Restoring deleted v1 or source-specific code as a compatibility layer.

## Minimum Contract Set

### Input DTOs/Commands

- `IndexSchema`, `IndexRecord`, `IndexMutation`, and `IndexQuery` are the current
  input contracts.
- `IndexQueryScope` carries tenant and locale independently from caller filters.
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
- Later milestones add source, retry, cancellation, and rebuild errors.
