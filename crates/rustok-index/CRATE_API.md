# rustok-index / CRATE_API

## Public Modules

- `domain`

The active engine contract is database independent. Source-specific Content,
Product, Flex, search, migration, runtime, and scheduler modules have been
deleted.

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

## Contract Status

The generic domain types are the only intentional engine contract at this
stage. Storage, source, ingestion, rebuild, query-port, and operator APIs are
published by their corresponding milestones.

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
- Restoring deleted v1 or source-specific code as a compatibility layer.

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
- Locale identity is explicit and is canonicalized before persistence.
- Field and link paths are validated against the schema registry.
- Source versions prevent stale mutation overwrite.
- Generic engine types remain source-domain agnostic.

### Events / Outbox Side Effects

- Source events are converted to `IndexMutation` through owner-published
  adapters.
- Delivery is replayable and idempotent.
- Mutation application uses inbox deduplication and transactional storage.

### Errors / Failure Codes

- `DomainError` defines current domain-shape validation failures.
- Application and infrastructure milestones add typed schema, planning, storage,
  source, retry, cancellation, and rebuild errors.
- Transport adapters preserve stable error classes without leaking database
  details.
