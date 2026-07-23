# rustok-index / CRATE_API

## Public Modules

- `domain`

The source-specific `content`, `product`, and `flex` implementation remains
internal during M0 and is scheduled for deletion. It is not a supported public
contract.

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

The database-independent domain types are the only intentional engine contract
at this stage. Storage, source, ingestion, rebuild, query-port, and operator APIs
will be published by their corresponding milestones.

Legacy `IndexDocument`, `DocumentType`, `IndexReadModelPort`,
`IndexRebuildPort`, in-memory fallback adapters, and source-specific query DTOs
have been deleted and must not be restored.

## Dependencies on Other RusToK Crates

The generic domain core must not depend on source-domain crates. `rustok-core`
is allowed for module metadata and shared platform primitives. Source adapters
belong to their owner modules or to explicit integration crates.

## Common AI Mistakes

- Adding Product, Content, Flex, Pricing, or Inventory fields to engine-core
  enums or structs.
- Reading source-module tables from Index.
- Treating Index as a ranking or full-text search engine.
- Reintroducing a catch-all JSON document as the public contract.
- Implementing rebuild by collecting every source ID before processing.
- Publishing unvalidated JSON filters instead of the typed query AST.

## Minimum Contract Set

### Input DTOs/Commands

- `IndexSchema`, `IndexRecord`, `IndexMutation`, and `IndexQuery` are the current
  input contracts.
- Construction and validation must preserve tenant, schema, entity, locale, and
  source-version identity.
- Public field changes are breaking until explicit versioning rules are added.

### Domain Invariants

- Every record and mutation is tenant scoped.
- Every record belongs to a registered schema and entity identity.
- Locale identity is explicit and will be canonicalized before persistence.
- Field and link paths must be validated against the schema registry.
- Source versions must prevent stale mutation overwrite.
- Generic engine types must remain source-domain agnostic.

### Events / Outbox Side Effects

- Incremental source events will be converted to `IndexMutation` through
  owner-published adapters.
- Delivery must be replayable and idempotent.
- Mutation application will use inbox deduplication and transactional storage.

### Errors / Failure Codes

- `DomainError` defines current domain-shape validation failures.
- Application and infrastructure milestones will add typed schema, planning,
  storage, source, retry, cancellation, and rebuild errors.
- Transport adapters must preserve stable error classes without leaking internal
  database details.
