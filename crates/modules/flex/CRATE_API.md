# flex / CRATE_API

## Public surface

### Primary Public Types and Signatures

- `pub struct FlexModule`: Native RusToK module engine implementing `RusToKModule` and `MigrationSource`.
- `pub mod attached`: Attached-mode field definitions, policies, entity references, and value payload resolution.
- `pub mod attached_definitions`: Definitions for attached custom fields bound to core entities.
- `pub mod attached_field_policy`: Policy enforcement for attached field values and validation rules.
- `pub mod standalone`: Standalone flex schemas, entries, and schema-driven entity instances.
- `pub mod standalone_definitions`: Schema field definition contracts for standalone entities.
- `pub mod standalone_field_policy`: Standalone field policy resolution, validation, and storage.
- `pub mod standalone_translation`: Localized field translation port, snapshot, and progress contracts.
- `pub mod standalone_translation_changes`: Translation journal change tracking and resource state.
- `pub mod events`: Domain events for schema and entry lifecycle transitions.
- `pub mod graphql`: Root schema queries and mutations for Flex capability.

## Platform Capability Bindings

- **RusToKModule Implementation**: `FlexModule` in `src/module.rs`
- **Module Slug**: `flex`
- **Module Name**: `Flex`
- **Permissions**: `FLEX_SCHEMAS_CREATE`, `FLEX_SCHEMAS_READ`, `FLEX_SCHEMAS_UPDATE`, `FLEX_SCHEMAS_DELETE`, `FLEX_SCHEMAS_LIST`, `FLEX_SCHEMAS_MANAGE`, `FLEX_ENTRIES_CREATE`, `FLEX_ENTRIES_READ`, `FLEX_ENTRIES_UPDATE`, `FLEX_ENTRIES_DELETE`, `FLEX_ENTRIES_LIST`, `FLEX_ENTRIES_MANAGE`
- **MigrationSource**: Declares migrations with explicit `MigrationDependencyDescriptor` and `MigrationSafetyMetadata`.

## Domain Collaborations

- **`rustok-core`**: Module lifecycle, migration safety metadata, and runtime extension contracts.
- **`rustok-events`**: Transactional domain events for flex schema and entry mutations.
- **`rustok-translation-targets`**: Standalone and attached translation target provider integration.
- **`rustok-product`**: Attached flex fields integration on product catalog entities.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateSchemaCommand`: Target schema slug, name, field descriptors, and tenant context.
- `UpdateSchemaCommand`: Schema updates, field additions, or deprecations.
- `CreateEntryCommand`: Schema identifier, dynamic field values map, and tenant context.
- `UpdateEntryCommand`: Target entry ID, field value modifications, and expected revision.
- `AttachedPayloadResolutionInput`: Raw payload input for resolving and validating attached field values.

### Domain Invariants

- **Strict Tenant Isolation**: All schema definitions, entries, and values are strictly partitioned by tenant ID without cross-tenant bleed.
- **Field Type Invariants**: Values must conform to the defined field type (text, number, boolean, json, date, relation) before persistence.
- **Migration Safety Classes**: All migrations declare explicit safety constraints (`ExpandContract`, `PreActivation`).
- **Translation Clock Isolation**: Localized fields maintain exact change journal clocks decoupled from host persistence timestamps.

### Events / Outbox Side Effects

- Schema creation, update, and deletion emit typed outbox events (`flex_schema_created_event`, `flex_schema_updated_event`, `flex_schema_deleted_event`).
- Entry mutations emit typed outbox events (`flex_entry_created_event`, `flex_entry_updated_event`, `flex_entry_deleted_event`).

### Errors / Failure Codes

- `FlexError::NotFound`: Requested schema, entry, or field does not exist.
- `FlexError::Validation`: Input payload failed schema type validation or constraint.
- `FlexError::Conflict`: Duplicate slug or concurrent revision conflict.
- `FlexStandaloneFieldPolicyError`: Failure resolving or applying field policy constraints.
