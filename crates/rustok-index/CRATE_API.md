# rustok-index / CRATE_API

## Public Modules

- `domain`

The active engine contract is database independent. Source-specific Content,
Product, Flex, search, and migration modules have been deleted.

## Primary Public Types

- `IndexModule`
- `ModuleName`, `SchemaRef`, `SchemaVersion`, `EntityName`, `EntityKey`
- `FieldName`, `FieldPath`, `LinkName`, `LocaleKey`
- `IndexValue`, `IndexValueType`
- `IndexSchema`, `IndexField`, `IndexLink`
- `IndexRecord`, `IndexLinkValue`, `LinkedEntityKey`
- `IndexMutation`
- `IndexQuery`, `FilterExpr`, `OrderExpr`, `OrderDirection`, `Pagination`
- `DomainError`

`Indexer`, `LocaleIndexer`, `IndexerContext`, `IndexerRuntimeConfig`,
`IndexError`, and `IndexResult` are temporary M0 compatibility exports only.
They are not part of the target API and will be removed with the server runtime
configuration tail.

## Contract Status

Storage, source, ingestion, rebuild, query-port, and operator APIs will be
published by their corresponding milestones. No compatibility contract exists
for deleted source-specific behavior.

Legacy `IndexDocument`, `DocumentType`, v1 read/rebuild ports, fallback adapters,
source query DTOs, indexers, projection models, and migrations must not return.

## Dependencies on Other RusToK Crates

The generic engine core must not depend on source-domain crates. `rustok-core`
is allowed for module metadata and shared platform primitives. Source adapters
belong to owner modules or explicit integration crates.

## Common AI Mistakes

- Adding Product, Content, Flex, Pricing, or Inventory fields to engine-core
  enums or structs.
- Reading source-module tables from Index.
- Treating Index as a ranking/full-text search engine.
- Reintroducing a catch-all JSON document as the public contract.
- Implementing rebuild by collecting every source ID before processing.
- Publishing unvalidated JSON filters instead of the typed query AST.
- Restoring deleted v1 ports or source indexers as compatibility code.

## Minimum Contract Set

### Input DTOs/Commands

- `IndexSchema`, `IndexRecord`, `IndexMutation`, and `IndexQuery` are the current
  input contracts.
- Construction and validation preserve tenant, schema, entity, locale, and
  source-version identity.
- Public field changes are breaking until explicit versioning rules are added.

### Domain Invariants

- Every record and mutation is tenant scoped.
- Every record belongs to a registered schema and entity identity.
- Locale identity is explicit and will be canonicalized before persistence.
- Field and link paths are validated against the schema registry.
- Source versions prevent stale mutation overwrite.
- Generic engine types remain source-domain agnostic.

### Events / Outbox Side Effects

- Source events are converted to `IndexMutation` through owner-published
  adapters.
- Delivery is replayable and idempotent.
- Mutation application will use inbox deduplication and transactional storage.

### Errors / Failure Codes

- `DomainError` defines current domain-shape validation failures.
- Application and infrastructure milestones add typed schema, planning, storage,
  source, retry, cancellation, and rebuild errors.
- Transport adapters preserve stable error classes without leaking database
  details.
