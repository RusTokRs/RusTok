# Flex — Custom Fields System

> **Attached mode status:** Phases 0–4 implemented. Phase 4.5 (extraction to `crates/flex`) — in progress.
> **Standalone mode status:** Phase 5 — in active implementation (transport-agnostic contracts, persistence/service layer, GraphQL and REST surfaces already live; rollout/governance contract already fixed, full integration verification remains as separate verification debt).
> **Module-system wiring status:** Phase 4.6 is now live: `flex` is registered in `modules.toml` as a `capability_only` ghost module with `rustok-module.toml` and runtime `FlexModule`; GraphQL roots/runtime/DTO and REST contract DTO belong to `crates/flex`, while `apps/server` remains the HTTP/persistence/composition adapter.
> Not implemented → [`implementation-plan.md`](./implementation-plan.md)

> **Important regarding multilingual support:** a common platform contract already applies for `flex`. `FieldDefinition.is_localized` is a live part of the DB/runtime contract; standalone schema copy (`name`, `description`) is stored in `flex_schema_translations`; standalone entry values are now split into `flex_entries.data` (shared/non-localized payload) and `flex_entry_localized_values` (locale-aware payload per `entry_id + locale`); attached-mode locale-aware values have a canonical storage-path in `flex_attached_localized_values`, and shared entity/helpers for this path live in `crates/flex`. Live write/read path is already wired for `user`, `product`, `order` and `topic`; for `topic` the donor payload now lives in `forum_topics.metadata`, and locale-aware Flex values go into parallel attached rows under the same contract.
> Cleanup/backfill of residual inline locale-aware payloads must be done via migrations; the runtime path must not read donor/base-row inline localized JSON as a canonical fallback.

---

## Purpose

`flex` defines the custom fields capability layer for RusToK: attached-mode extends donor modules, while standalone mode provides schema/entry runtime for extension scenarios without turning `flex` into a separate business bounded context.

## Scope

- transport-agnostic field definition contracts, owner-owned row-to-core/view-source/command conversion mapping, persisted JSON shape helpers, lifecycle guardrails/events, cache invalidation event taxonomy and registry/orchestration helpers;
- owner-owned attached field-definition and standalone GraphQL query/mutation roots, runtime handle and input/output DTO without server dependencies;
- owner-owned standalone REST request/response DTO, request-to-command mapping and view mapping in `flex::rest`; server controller remains only a Loco/Axum adapter;
- owner-owned standalone fields_config parsing/schema building/serialization, localized field-key derivation, row-to-view mapping, entry normalize/defaults/strip/validate helper, shared/localized split, read resolution and PATCH merge helpers; server persistence adapter only exposes source traits, delegates Flex contract helpers and performs storage operations;
- multilingual storage/runtime contract for attached and standalone Flex payload;
- capability-only module metadata for `modules.toml` / `rustok-module.toml` / `ModuleRegistry`;
- rules by which donor persistence ownership stays with consumer modules.

## Integration

- `rustok-core::field_schema` supplies base types, validation rules and migration helpers;
- `crates/flex` holds shared attached/standalone contracts and runtime metadata via `FlexModule`;
- `apps/server` remains the adapter/composition layer for SeaORM, REST handler and bootstrap; attached field-definition row-to-core/view/command mapping, create guardrails, persisted JSON shape helpers, persisted type-name normalization, lifecycle event construction and cache invalidation event taxonomy live in `flex::registry`, attached field-definition and standalone GraphQL roots/runtime/DTO/RBAC/error/event mapping live in `flex::graphql`, REST request/response DTO, request-to-command mapping and view mapping live in `flex::rest`, roots are connected via `[provides.graphql]`, and the host passes concrete standalone service, registry/cache and DB handle through `FlexGraphqlRuntime`;
- donor write/read paths are currently live for `user`, `product`, `order` and `topic`.

## Verification

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`

## Related documents

- [`implementation-plan.md`](./implementation-plan.md)
- [`../README.md`](../README.md)
- [`../../../docs/modules/manifest.md`](../../../docs/modules/manifest.md)
- [`../../../docs/architecture/database.md`](../../../docs/architecture/database.md)

---

## 0. Current Architecture Boundary

Canonical Flex module documentation lives in this file.

The current architecture is divided into three layers:

- `rustok-core::field_schema` stores base types, validators and migration helpers for attached mode;
- `crates/flex` stores transport-agnostic orchestration, registry, field-definition row-to-core/view-source/command conversion mapping, persisted JSON shape helpers, attached field-definition lifecycle guardrails/events/cache invalidation taxonomy, standalone contracts, standalone fields_config/schema/key-derivation/row-view/entry validation/split/merge helpers, attached/standalone GraphQL roots/runtime/DTO and REST contract DTO/command mapping;
- `apps/server` holds the adapter/wiring layer: SeaORM, REST handler, cache/bootstrap and schema runtime registration. Owner roots come in via manifest codegen; concrete `FlexStandaloneSeaOrmService`, `FieldDefRegistry`, DB handle and cache adapter are created/passed only in the composition root through `FlexGraphqlRuntime`.

Attached mode is considered a working production contract. Standalone mode already has live GraphQL and REST API surfaces in `apps/server`; rollout/governance policy for this surface is now also fixed, and only the full integration verification remains open.

`flex` is not considered a regular bounded-context module. In the module system it lives as a capability-only ghost module: registry/RBAC/runtime metadata are formalized by the manifest layer, but donor persistence ownership stays with consumer modules.

---

## 1. What is Flex

**Flex is a katana, not a warehouse of swords.**

Flex is a library module: a set of types, validators and migration helpers inside `rustok-core` that allows **any module** to add runtime-defined custom fields with minimal code.

Flex exists **alongside** standard modules, not **instead of** them. It is an "emergency exit" for edge-cases:
- Standard domain modules (content, commerce, blog) are insufficient
- Creating a separate domain module is not practical
- Business wants custom fields without programming

### What Flex is for

✅ Custom fields for existing entities (attached mode)
✅ Runtime-defined data schemas
✅ Storing additional data in JSONB
✅ Marketing landing pages, forms, directories (standalone mode, Phase 5)

### What Flex is NOT for

❌ Replacement for standard modules (content, commerce, blog)
❌ Storing critical data (orders, payments, inventory)
❌ Creating complex relational links
❌ Alternative to normalized tables

---

## 2. Architecture Laws (HARD LAWS)

| # | Rule | Rationale |
|---|------|-----------|
| 1 | **Standard modules NEVER depend on Flex** | Flex is an option, not a dependency |
| 2 | **Flex depends only on `rustok-core`** | Modules depend only on core |
| 3 | **Removal-safe** | Remove `field_schema.rs` — platform works (loses custom fields) |
| 4 | **Data stays in the module** | Tables and metadata JSONB in the consumer module |
| 5 | **Schema-first** | All data is validated against schema on write |
| 6 | **Tenant isolation** | Field definitions per-tenant |
| 7 | **No Flex in critical domains** | Orders/payments/inventory — normalized fields |

```text
rustok-core  ←  all depend on it
    ↑
field_schema.rs (type library)

rustok-commerce ←✗→ flex  (NO dependency on flex!)
```

---

## 3. Two Modes

### Attached mode (implemented, Phases 0–4)

Custom fields are attached to existing entities via donor payload and, for `is_localized=true`,
via parallel localized records:

```
"Give me custom fields for users"
  → user_field_definitions (definitions table)
  + users.metadata (shared / non-localized data)
  + flex_attached_localized_values (locale-aware attached values)
```

### Standalone mode (Phase 5, live GraphQL surface + incomplete rollout)

Arbitrary schemas and entries without binding to existing entities:

```
"Give me an arbitrary entity 'landing-page'"
  → flex_schemas (schema definition)
  + flex_entries (shared/non-localized record)
  + flex_entry_localized_values (locale-aware payload per entry)
```

Both modes use the same type library from `rustok-core::field_schema`.

---

## 4. Core types (`rustok-core/src/field_schema.rs`)

### 4.1 FieldType — 14 Field Types

```rust
pub enum FieldType {
    Text,        // Single-line text
    Textarea,    // Multi-line text
    Integer,     // i64
    Decimal,     // f64
    Boolean,     // true/false
    Date,        // ISO 8601 date (YYYY-MM-DD)
    DateTime,    // ISO 8601 date-time
    Url,         // URL with format validation
    Email,       // Email with format validation
    Phone,       // Phone (with optional regex)
    Select,      // Single option from a list
    MultiSelect, // Multiple options (array)
    Color,       // Hex color (#RRGGBB)
    Json,        // Arbitrary JSON (with depth guardrail)
}
```

### 4.2 Validation Rules by Type

| FieldType   | JSON type        | min/max              | pattern | options |
|-------------|------------------|----------------------|---------|---------|
| Text        | String           | string length        | ✅      | —       |
| Textarea    | String           | string length        | ✅      | —       |
| Integer     | Number (i64)     | numeric value        | —       | —       |
| Decimal     | Number (f64)     | numeric value        | —       | —       |
| Boolean     | Boolean          | —                    | —       | —       |
| Date        | String (ISO)     | —                    | —       | —       |
| DateTime    | String (ISO)     | —                    | —       | —       |
| Url         | String           | string length        | —       | —       |
| Email       | String           | string length        | —       | —       |
| Phone       | String           | string length        | ✅      | —       |
| Select      | String           | —                    | —       | ✅      |
| MultiSelect | Array\<String\>  | element count        | —       | ✅      |
| Color       | String (#RRGGBB) | —                    | —       | —       |
| Json        | Any              | — (see depth)        | —       | —       |

### 4.3 ValidationRule and SelectOption

```rust
pub struct ValidationRule {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub pattern: Option<String>,
    pub options: Option<Vec<SelectOption>>,
    pub error_message: Option<HashMap<String, String>>, // localized messages
}

pub struct SelectOption {
    pub value: String,
    pub label: HashMap<String, String>, // {"en": "Active", "ru": "Active_RU"}
}
```

### 4.4 FieldDefinition — Portable DTO

```rust
pub struct FieldDefinition {
    pub field_key: String,                              // snake_case, unique per tenant+entity scope
    pub field_type: FieldType,
    pub label: HashMap<String, String>,                 // {"en": "Phone", "ru": "Phone_RU"}
    pub description: Option<HashMap<String, String>>,
    pub is_localized: bool,
    pub is_required: bool,
    pub default_value: Option<serde_json::Value>,
    pub validation: Option<ValidationRule>,
    pub position: i32,
    pub is_active: bool,
}
```

### 4.5 CustomFieldsSchema — Validator

```rust
impl CustomFieldsSchema {
    /// Build a schema from a list of definitions (from DB, config, JSONB)
    pub fn new(definitions: Vec<FieldDefinition>) -> Self;

    /// Validate metadata. Empty list = valid.
    pub fn validate(&self, metadata: &serde_json::Value) -> Vec<FieldValidationError>;

    /// Fill default_value for missing fields
    pub fn apply_defaults(&self, metadata: &mut serde_json::Value);

    /// Remove fields not in the schema
    pub fn strip_unknown(&self, metadata: &mut serde_json::Value);

    /// Only active definitions in position order
    pub fn active_definitions(&self) -> Vec<&FieldDefinition>;
}
```

### 4.6 HasCustomFields trait

```rust
pub trait HasCustomFields {
    fn entity_type() -> &'static str;          // "user", "product", "topic"
    fn metadata(&self) -> &serde_json::Value;
    fn set_metadata(&mut self, value: serde_json::Value);
}
```

### 4.7 Migration helper

```rust
    /// Create a `{prefix}_field_definitions` table in any module's migration.
    /// One line of code — and the module gets a full custom fields table.
pub async fn create_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,       // "user" → creates "user_field_definitions"
    _parent_table: &str,
) -> Result<(), DbErr>;

pub async fn drop_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,
) -> Result<(), DbErr>;
```

Creates a table with columns: `id`, `tenant_id`, `field_key`, `field_type`, `label`,
`description`, `is_required`, `default_value`, `validation`, `position`, `is_active`,
`created_at`, `updated_at`.
Indexes: `UNIQUE(tenant_id, field_key)`, `idx_{prefix}_fd_tenant_active`.

### 4.8 SeaORM entity macro

```rust
/// Generates a SeaORM entity for the field_definitions table in one line.
rustok_core::define_field_definitions_entity!("user_field_definitions");
// Generates: Entity, Model, ActiveModel, Column, Relation, PrimaryKey
```

### 4.9 JSONB query helpers

```rust
/// metadata->>'key' = 'value'
pub fn json_field_eq(column, key: &str, value: &str) -> Condition;

/// metadata ? 'key'  (key exists)
pub fn json_field_exists(column, key: &str) -> Condition;

/// metadata->>'key'  (for ORDER BY)
pub fn json_field_extract(column, key: &str) -> SimpleExpr;

/// metadata @> '{"key": value}'  (contains)
pub fn json_field_contains(column, key: &str, value: serde_json::Value) -> Condition;
```

---

## 5. Guardrails

| Guardrail | Value | Status | Where checked |
|-----------|-------|--------|---------------|
| Max standalone fields per schema | **50** | ✅ implemented for standalone | `validate_create_schema_command()` / `validate_update_schema_command()` before adapter/service layer |
| Standalone schema slug length | **64** chars | ✅ implemented for standalone | `validate_create_schema_command()` before writing to `flex_schemas.slug VARCHAR(64)` |
| Standalone schema name length | **255** chars | ✅ implemented for standalone | create/update validators before writing to `flex_schema_translations.name VARCHAR(255)` |
| Standalone entry relation type length | **64** chars | ✅ implemented for standalone | `validate_create_entry_command()` before writing to `flex_entries.entity_type VARCHAR(64)` |
| Standalone entry status format and length | `^[a-z][a-z0-9_]*$`, **32** chars | ✅ implemented for standalone | create/update entry validators before writing to `flex_entries.status VARCHAR(32)` |
| Max nesting depth (`FieldType::Json`) | **2** | ✅ implemented | `validate_field_value()` |
| Validation on write | **Strict** | ✅ implemented | `CustomFieldsSchema::validate()` |
| `field_key` format | `^[a-z][a-z0-9_]{0,127}$` | ✅ implemented | `is_valid_field_key()` |
| Locale key format | BCP 47 short | ✅ implemented | `is_valid_locale_key()` |
| Mandatory pagination | Yes | ✅ implemented | `fieldDefinitions` GraphQL query |
| Timeout for JSONB operations | 5s | ⬜ TODO | DB query timeout |

### 5.1 JSON Depth Counting Method — Variant A (arrays are transparent)

For `FieldType::Json`, only JSON object levels `{…}` are counted. Arrays `[…]` do not create a level.

| Value | Object-depth | Allowed? |
|----------|-------------|-----------|
| `42` / `"hello"` / `true` / `null` | 0 | ✅ |
| `[1, 2, 3]` | 0 | ✅ |
| `{"key": "value"}` | 1 | ✅ |
| `{"items": [1, 2, 3]}` | 1 | ✅ |
| `{"address": {"city": "NY"}}` | 2 | ✅ (boundary) |
| `{"items": [{"id": 1, "name": "x"}]}` | 2 | ✅ (array is transparent) |
| `{"a": {"b": {"c": 1}}}` | 3 | ❌ `NestingTooDeep` |

**Why Variant A:** the pattern `{"items": [{"id":1}]}` appears constantly in any CMS. When counting arrays it would give depth=3 and be blocked without benefit. Variant A with limit 2 specifically prohibits triple object nesting.

**Implementation:** `json_object_depth()` + `MAX_JSON_NESTING_DEPTH = 2` in `rustok-core/src/field_schema.rs`.
**Error:** `FieldErrorCode::NestingTooDeep` with current depth and limit in message.

---

## 6. How to Connect Flex to a Module (5 Steps)

Each module = ~50 lines of new code. Everything else is in core.

### Step 1: Migration

```rust
// crates/rustok-migrations/src/m20260315_000001_create_user_field_definitions.rs
use rustok_core::field_schema::create_field_definitions_table;

async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    create_field_definitions_table(manager, "user", "users").await
}
async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    drop_field_definitions_table(manager, "user").await
}
```

### Step 2: SeaORM Entity

```rust
rustok_core::define_field_definitions_entity!("user_field_definitions");
```

### Step 3: HasCustomFields for the Entity

```rust
impl HasCustomFields for user::Model {
    fn entity_type() -> &'static str { "user" }
    fn metadata(&self) -> &serde_json::Value { &self.metadata }
    fn set_metadata(&mut self, value: serde_json::Value) { self.metadata = value.into(); }
}
```

### Step 4: Service

```rust
pub async fn get_schema(db: &DatabaseConnection, tenant_id: Uuid) -> Result<CustomFieldsSchema> {
    let rows = user_field_definitions::Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::IsActive.eq(true))
        .order_by_asc(Column::Position)
        .all(db).await?;

    Ok(CustomFieldsSchema::new(rows.into_iter().map(|r| r.into_field_definition()).collect()))
}
```

### Step 5: Validation in Mutations

```rust
let schema = UserFieldService::get_schema(db, tenant_id).await?;
let mut metadata = input.custom_fields.unwrap_or(json!({}));
schema.apply_defaults(&mut metadata);
let errors = schema.validate(&metadata);
if !errors.is_empty() {
    return Err(custom_field_validation_error(errors));
}
```

---

## 7. Current Consumers (attached mode)

| Module | Table | entity_type | donor payload |
|--------|---------|-------------|---------------|
| apps/server | `user_field_definitions` | `"user"` | `users.metadata` + `flex_attached_localized_values` |
| apps/server + `crates/flex` | `product_field_definitions` | `"product"` | `products.metadata` + `flex_attached_localized_values` |
| apps/server + `crates/flex` | `order_field_definitions` | `"order"` | `orders.metadata` + `flex_attached_localized_values` |
| apps/server + `crates/flex` | `topic_field_definitions` | `"topic"` | `forum_topics.metadata` + `flex_attached_localized_values` |

All definitions tables are structurally identical, physically isolated in their own module. For attached localized values, canonical shared storage now lives in `flex_attached_localized_values`, and shared entity/helpers are moved to `crates/flex`; `user`, `product`, `order` and `topic` already use this path in the live read/write flow.

---

## 8. Admin API (GraphQL)

### Queries

```graphql
fieldDefinitions(entityType: String, pagination: PaginationInput!): [FieldDefinition!]!
fieldDefinition(entityType: String, id: UUID!): FieldDefinition
```

### Mutations

```graphql
createFieldDefinition(input: CreateFieldDefinitionInput!): FieldDefinition!
updateFieldDefinition(id: UUID!, input: UpdateFieldDefinitionInput!): FieldDefinition!
deleteFieldDefinition(entityType: String, id: UUID!): DeleteFieldDefinitionPayload!
reorderFieldDefinitions(entityType: String, ids: [UUID!]!): [FieldDefinition!]!
```

### Routing by entityType

Requests are routed through `FieldDefRegistry` — modules register their repositories at startup:

```rust
let mut registry = FieldDefRegistry::new();
registry.register(Box::new(UserFieldRepo));
registry.register(Box::new(ProductFieldRepo));
// ...

// In resolver:
let repo = registry.get(entity_type)?; // → FlexError::UnknownEntityType if not found
```

### RBAC

| Surface | Typed permissions |
|---------|-------------------|
| Attached field definitions query roots | `flex_schemas:list`, `flex_schemas:read` |
| Attached field definitions mutations | `flex_schemas:create`, `flex_schemas:update`, `flex_schemas:delete` |
| Standalone schema queries/mutations | `flex_schemas:*` |
| Standalone entry queries/mutations | `flex_entries:*` |

Typed permission checks go through `require_permission(...)` in GraphQL and `RequireFlex*` extractors in the REST adapter layer.
Attached custom field filling remains tied to the donor write-path and its own entity permissions.

---

## 9. Events

### Emitted Events

```rust
DomainEvent::FieldDefinitionCreated { tenant_id, entity_type, field_key, field_type }
DomainEvent::FieldDefinitionUpdated { tenant_id, entity_type, field_key }
DomainEvent::FieldDefinitionDeleted { tenant_id, entity_type, field_key }
```

### Consumers

```rust
// Schema cache invalidation
FieldDefinitionCreated | FieldDefinitionUpdated | FieldDefinitionDeleted => {
    schema_cache.invalidate(tenant_id, entity_type);
}

// Audit
FieldDefinition* => {
    audit_logger.log(AuditEventType::ConfigurationChange, ...);
}
```

### Cascade policy

- Entity deletion (user, product) → metadata is deleted together (CASCADE at row level)
- Soft delete field definition (`is_active=false`) → data in metadata is not touched
- Hard delete field definition → `strip_unknown()` on next write

---

## 10. Schema Caching

```rust
const SCHEMA_CACHE_TTL: Duration = Duration::from_secs(300); // safety net

/// Per (tenant_id, entity_type) cache.
/// Primary invalidation: via FieldDefinition* events.
/// Secondary: TTL as safety net.
pub struct SchemaCache {
    inner: DashMap<(Uuid, String), (Instant, CustomFieldsSchema)>,
}
```

Implementation: Moka cache + event-driven invalidation on mutations + listener on `FieldDefinition*` EventBus events. In the agnostic layer, helpers `list_field_definitions_with_cache()` and `invalidate_field_definition_cache()` + port `FieldDefinitionCachePort` are available.

---

## 11. Error Handling

```rust
pub enum FlexError {
    UnknownEntityType(String),                        // → "UNKNOWN_ENTITY_TYPE"
    TooManyFields { entity_type: String, max: usize },// → "TOO_MANY_FIELDS"
    InvalidFieldKey(String),                          // → "BAD_USER_INPUT"
    DuplicateFieldKey(String),                        // → "BAD_USER_INPUT"
    NotFound(Uuid),                                   // → "NOT_FOUND"
    ValidationFailed(Vec<FieldValidationError>),       // → "VALIDATION_FAILED" + fields
    Database(String),                                 // → "INTERNAL_ERROR"
}
```

All errors are mapped through transport-agnostic `flex::map_flex_error()`; in GraphQL only adaptation to `FieldError` with corresponding codes in error extensions is performed.

---

## 12. Standalone mode (Phase 5 — GraphQL + REST live, rollout/governance contract fixed)

At the current stage, for standalone mode already live:

- `FlexSchemaView`, `FlexEntryView`
- `CreateFlexSchemaCommand`, `UpdateFlexSchemaCommand`
- `CreateFlexEntryCommand`, `UpdateFlexEntryCommand`
- `FieldDefinitionViewSource` + `FieldDefinitionView::from_source()` for owner-owned mapping from persisted field-definition rows to Flex view
- `FieldDefinitionSource` + `field_definition_from_source()` + `impl_field_definition_source!` for owner-owned mapping from persisted field-definition rows to core `FieldDefinition`
- `impl_field_definition_command_conversions!` for owner-owned mapping from Flex field-definition commands to adapter input structs
- `field_definition_label_json()`, `field_definition_description_json()` and `field_definition_validation_json()` for owner-owned persisted JSON shape label/description/validation
- `field_definition_cache_invalidation_target()` for owner-owned selection of events that invalidate attached field-definition cache
- `validate_field_definition_create()`, `field_definition_position_or_next()`, `field_definition_type_name()` and `field_definition_*_event()` for owner-owned lifecycle policy attached field definitions; server persistence adapters only perform SeaORM lookup/count/write and call these helpers
- `FlexStandaloneService`
- `normalize_and_validate_standalone_entry`
- `parse_standalone_fields_config`, `build_standalone_custom_fields_schema`, `serialize_standalone_fields_config` and `standalone_localized_field_keys`
- `StandaloneSchemaViewSource`, `StandaloneSchemaTranslationSource`, `StandaloneEntryViewSource`, `standalone_schema_view_from_source` and `standalone_entry_view_from_source`
- Guardrail validators: `validate_create_schema_command`, `validate_update_schema_command`, `validate_create_entry_command`, `validate_update_entry_command` now check JSON-object form for payload, normalized identifiers/statuses/schema names, limit of 50 fields per schema and DB-column length caps for schema slugs/names, as well as entry `entity_type`/`status`.
- Orchestration helpers: `list/find/create/update/delete` for schemas and entries
- GraphQL queries/mutations in `crates/flex/src/graphql` for schemas and entries with shared `AuthContext` / `TenantContext` and separate `flex_schemas:*` / `flex_entries:*` permission gates
- REST endpoints in `apps/server`: `/api/v1/flex/schemas*` and `/api/v1/flex/schemas/{schema_id}/entries*` with the same tenant-scoped RBAC gates; request/response DTO, command mapping and view mapping come from `flex::rest`

A live rollout/governance contract already applies for the standalone surface:

- attached field-definition and standalone GraphQL transport belong to `crates/flex`, roots are connected via manifest codegen, REST contract DTO belong to `flex::rest`, and the server only registers runtime, concrete persistence/registry/cache adapters and Loco/Axum REST handler;
- `flex` is registered in `modules.toml` as a `capability_only` ghost module with `rustok-module.toml` and runtime `FlexModule`;
- capability wiring is verified via `cargo xtask validate-manifest` and `cargo xtask module validate flex`;
- multilingual DB/runtime drift is verified via `node scripts/verify/verify-flex-multilingual-contract.mjs`;
- access to schemas/entries goes only through tenant-scoped `flex_schemas:*` and `flex_entries:*` permission gates;
- the server remains the canonical validator of lifecycle and transport policy; thin clients do not introduce their own rollout/governance contract locally.

Full integration verification remains open; the follow-up backlog no longer includes `indexer/cascade-delete`, but still includes expanding test coverage and further evolution of the standalone surface.

### Data model

```sql
-- Arbitrary schema definitions
CREATE TABLE flex_schemas (
    id          UUID PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug        VARCHAR(64) NOT NULL,       -- 'landing-page', 'feedback-form'
    fields_config JSONB NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (tenant_id, slug)
);

CREATE TABLE flex_schema_translations (
    schema_id    UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    locale       VARCHAR(32) NOT NULL,
    name         VARCHAR(255) NOT NULL,
    description  TEXT,
    PRIMARY KEY (schema_id, locale)
);

-- Data records
CREATE TABLE flex_entries (
    id          UUID PRIMARY KEY,
    tenant_id   UUID NOT NULL,
    schema_id   UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    entity_type VARCHAR(64),               -- NULL = standalone
    entity_id   UUID,                      -- NULL = standalone
    data        JSONB NOT NULL,
    status      VARCHAR(32) NOT NULL DEFAULT 'draft'
);
CREATE INDEX idx_flex_entries_data   ON flex_entries USING GIN (data);
CREATE INDEX idx_flex_entries_entity ON flex_entries (entity_type, entity_id);

CREATE TABLE flex_entry_localized_values (
    entry_id     UUID NOT NULL REFERENCES flex_entries(id) ON DELETE CASCADE,
    locale       VARCHAR(32) NOT NULL,
    tenant_id    UUID NOT NULL,
    data         JSONB NOT NULL DEFAULT '{}',
    PRIMARY KEY (entry_id, locale)
);
CREATE INDEX idx_flex_entry_localized_values_owner
    ON flex_entry_localized_values (tenant_id, entry_id);
```

### Guardrails standalone mode

- Max relation depth = 1 (no recursive populate)
- Schema slugs fit in `VARCHAR(64)`, schema names — in `VARCHAR(255)`, entry relation types — in `VARCHAR(64)`, entry statuses — in `VARCHAR(32)`.
- Entry statuses must be pre-normalized machine identifiers (`^[a-z][a-z0-9_]*$`) without surrounding whitespace.
- FlexEntry A can reference User/Product ✅
- FlexEntry A → FlexEntry B → FlexEntry C ❌

Implementation details — in [`implementation-plan.md`](./implementation-plan.md).

---

## See Also

- [`implementation-plan.md`](./implementation-plan.md) — not yet implemented (Phase 4 debts, Phase 4.5, 5, 6)
- [`rustok-core/src/field_schema.rs`](../../crates/rustok-core/src/field_schema.rs) — source code of core types
- [`../../../docs/modules/_index.md`](../../../docs/modules/_index.md) — central module documentation index
