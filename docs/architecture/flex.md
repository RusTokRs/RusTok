# Flex — Implementation Plan

> **Статус:** Draft v4 (2026-03-14)
> **Архитектурное решение:** Flex — модуль-библиотека (набор типов, валидаторов, хелперов) внутри `rustok-core`.

---

## 1. Философия

**Flex — это катана, а не склад мечей.**

Flex — модуль-библиотека: набор типов, валидаторов, хелперов для миграций и SeaORM,
которые позволяют **любому модулю** добавить кастомные поля за минимум кода.
Сейчас не имеет своих таблиц — данные живут в модуле-потребителе.
В будущем может получить собственные таблицы (standalone mode).

Когда модуль подключает Flex, происходит следующее:
- В **миграциях модуля** появляется таблица `{entity}_field_definitions` (через хелпер)
- В **entity модуля** появляется `impl HasCustomFields` (через trait из core)
- В **мутациях модуля** появляется валидация metadata (через `CustomFieldsSchema`)
- **Данные остаются в модуле**, в его `metadata` JSONB колонке

Flex — это как `serde`: derive + trait, а реализация у каждого своя.

---

## 2. Архитектура: два слоя

```
┌─────────────────────────────────────────────────────────────────┐
│                     rustok-core (ядро)                          │
│                                                                 │
│  src/field_schema.rs                                           │
│  ├── FieldType enum (Text, Integer, Select, ...)               │
│  ├── ValidationRule { min, max, pattern, options }              │
│  ├── FieldDefinition (portable DTO)                            │
│  ├── CustomFieldsSchema (валидатор)                            │
│  │   ├── validate(metadata) → Vec<Error>                       │
│  │   ├── apply_defaults(metadata)                              │
│  │   └── strip_unknown(metadata)                               │
│  ├── HasCustomFields trait                                     │
│  ├── FieldValidationError, FieldErrorCode                      │
│  └── migration helpers:                                        │
│      └── create_field_definitions_table(manager, table, fk)    │
│                                                                 │
│  Аналоги в core: i18n.rs, types.rs — просто контракт.          │
└──────────────────────────┬──────────────────────────────────────┘
                           │ зависят от core
          ┌────────────────┼────────────────────┐
          │                │                    │
          ▼                ▼                    ▼
┌─────────────────┐ ┌──────────────┐  ┌─────────────────┐
│  apps/server    │ │ rustok-      │  │ rustok-         │
│  (users)        │ │ commerce     │  │ content         │
│                 │ │              │  │                 │
│  Migration:     │ │  Migration:  │  │  Migration:     │
│  user_field_    │ │  product_    │  │  node_field_    │
│  definitions    │ │  field_defs  │  │  definitions    │
│  (helper!)      │ │  (helper!)   │  │  (helper!)      │
│                 │ │              │  │                 │
│  users.metadata │ │  products.   │  │  nodes.metadata │
│  (JSONB)        │ │  metadata    │  │  (JSONB)        │
│                 │ │  (JSONB)     │  │                 │
│  impl Has       │ │  impl Has   │  │  impl Has       │
│  CustomFields   │ │  Custom...  │  │  CustomFields   │
│  for User       │ │  for Product│  │  for Node       │
└─────────────────┘ └──────────────┘  └─────────────────┘
```

### Ключевые свойства

| Свойство | Значение |
|----------|----------|
| Тип модуля | **Модуль-библиотека** внутри `rustok-core` |
| Имеет свои таблицы? | Сейчас нет (таблицы в модулях-потребителях). Standalone mode — будут |
| Модули зависят от Flex? | Зависят от `rustok-core` (который уже в deps) |
| Flex зависит от модулей? | **НЕТ** — не знает о конкретных сущностях |
| Удаление Flex ломает платформу? | **НЕТ** — модули просто теряют кастомные поля |
| Данные изолированы? | **ДА** — каждый модуль хранит своё |

### Архитектурные законы (HARD LAWS)

| # | Правило | Обоснование |
|---|---------|-------------|
| 1 | **Flex — модуль-библиотека внутри core** | Модули зависят только от core |
| 2 | **Данные остаются в модуле** | Таблицы и metadata в модуле-потребителе |
| 3 | **Removal-safe** | Удали field_schema.rs — платформа работает (теряются custom fields) |
| 4 | **No Flex in critical domains** | Orders/payments/inventory — нормализованные поля |
| 5 | **Schema-first** | Все данные валидируются по схеме при записи |
| 6 | **Tenant isolation** | Определения полей per-tenant |

---

## 3. Part 1 — `rustok-core/src/field_schema.rs`

### 3.1 FieldType enum

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported field types for custom fields.
/// Shared platform contract — used by any module that needs
/// runtime-defined field types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// Single-line text
    Text,
    /// Multi-line text
    Textarea,
    /// Integer number (i64)
    Integer,
    /// Decimal number (f64)
    Decimal,
    /// true/false
    Boolean,
    /// ISO 8601 date (YYYY-MM-DD)
    Date,
    /// ISO 8601 date-time
    DateTime,
    /// URL (format validated)
    Url,
    /// Email (format validated)
    Email,
    /// Phone (free-form with optional regex)
    Phone,
    /// Single select from options list
    Select,
    /// Multi select from options list
    MultiSelect,
    /// Color hex (#RRGGBB)
    Color,
    /// Arbitrary JSON
    Json,
}

impl FieldType {
    /// Returns true if this type requires `options` in ValidationRule.
    pub fn requires_options(&self) -> bool {
        matches!(self, Self::Select | Self::MultiSelect)
    }

    /// Returns true if this type supports min/max as string length.
    pub fn min_max_is_length(&self) -> bool {
        matches!(self, Self::Text | Self::Textarea | Self::Url | Self::Email | Self::Phone)
    }

    /// Returns true if this type supports regex pattern.
    pub fn supports_pattern(&self) -> bool {
        matches!(self, Self::Text | Self::Textarea | Self::Phone)
    }
}
```

### 3.2 ValidationRule & SelectOption

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<SelectOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    /// Localized labels: {"en": "Male", "ru": "Мужской"}
    pub label: HashMap<String, String>,
}
```

### 3.3 FieldDefinition (portable DTO)

```rust
/// Runtime field definition. Portable DTO that DB rows, config files,
/// and JSONB fields_config can all be converted into.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub field_key: String,
    pub field_type: FieldType,
    /// Localized labels: {"en": "Phone", "ru": "Телефон"}
    pub label: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<HashMap<String, String>>,
    #[serde(default)]
    pub is_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationRule>,
    #[serde(default)]
    pub position: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
}
```

### 3.4 HasCustomFields trait

```rust
/// Trait for entities that support custom fields via metadata JSONB column.
/// Each module implements this for its own entities.
pub trait HasCustomFields {
    /// Entity type key, e.g. "user", "product", "node".
    fn entity_type() -> &'static str;

    /// Current metadata as JSON.
    fn metadata(&self) -> &serde_json::Value;

    /// Set metadata.
    fn set_metadata(&mut self, value: serde_json::Value);
}
```

### 3.5 CustomFieldsSchema (валидатор)

```rust
/// Schema-based validator. Constructed from FieldDefinitions
/// loaded from any source (DB table, config file, JSONB column).
pub struct CustomFieldsSchema {
    definitions: Vec<FieldDefinition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldValidationError {
    pub field_key: String,
    pub message: String,
    pub error_code: FieldErrorCode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldErrorCode {
    Required,
    InvalidType,
    TooShort,
    TooLong,
    BelowMinimum,
    AboveMaximum,
    PatternMismatch,
    InvalidOption,
    InvalidFormat,
}

impl CustomFieldsSchema {
    pub fn new(definitions: Vec<FieldDefinition>) -> Self;

    /// Validate metadata. Returns errors (empty = valid).
    pub fn validate(&self, metadata: &serde_json::Value) -> Vec<FieldValidationError>;

    /// Fill in defaults for missing fields.
    pub fn apply_defaults(&self, metadata: &mut serde_json::Value);

    /// Remove fields not in schema.
    pub fn strip_unknown(&self, metadata: &mut serde_json::Value);

    /// Active definitions only.
    pub fn active_definitions(&self) -> Vec<&FieldDefinition>;
}
```

### 3.6 Migration helper — ключевая часть "Flex как катана"

```rust
use sea_orm_migration::prelude::*;

/// Helper to create a `{prefix}_field_definitions` table in any module's migration.
///
/// # Example
///
/// ```rust
/// // In apps/server/migration/src/m20260315_..._create_user_field_definitions.rs
/// use rustok_core::field_schema::create_field_definitions_table;
///
/// async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
///     create_field_definitions_table(
///         manager,
///         "user",        // prefix → creates "user_field_definitions"
///         "users",       // parent table (not used as FK — just for naming)
///     ).await
/// }
/// ```
///
/// Creates table with columns:
///   id, tenant_id, field_key, field_type, label, description,
///   is_required, default_value, validation, position, is_active,
///   created_at, updated_at
///
/// Creates indexes:
///   UNIQUE (tenant_id, field_key)
///   idx_{prefix}_fd_tenant_active (tenant_id, is_active)
pub async fn create_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,
    _parent_table: &str,
) -> Result<(), DbErr>;

/// Corresponding drop helper.
pub async fn drop_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,
) -> Result<(), DbErr>;
```

**Это то, что делает Flex "катаной":** одна функция — и модуль получает полноценную
таблицу кастомных полей с правильной структурой, индексами, tenant isolation.

### 3.7 SeaORM entity macro (опционально, Phase 2+)

```rust
/// Macro to generate SeaORM entity for a field_definitions table.
/// Eliminates boilerplate — каждый модуль получает entity за одну строку.
///
/// ```rust
/// rustok_core::define_field_definitions_entity!("user_field_definitions");
/// ```
///
/// Generates: Entity, Model, ActiveModel, Column, Relation, PrimaryKey
/// with the correct table name and standard column set.
macro_rules! define_field_definitions_entity { ... }
```

### 3.8 JSONB Query Helpers

```rust
/// Helpers for filtering/sorting entities by custom field values in metadata JSONB.
/// Database-agnostic (PostgreSQL JSONB operators).

/// metadata->>'key' = 'value'
pub fn json_field_eq(column: impl Into<SimpleExpr>, key: &str, value: &str) -> Condition;

/// metadata ? 'key' (key exists)
pub fn json_field_exists(column: impl Into<SimpleExpr>, key: &str) -> Condition;

/// metadata->>'key' (for ORDER BY)
pub fn json_field_extract(column: impl Into<SimpleExpr>, key: &str) -> SimpleExpr;

/// metadata @> '{"key": value}' (contains)
pub fn json_field_contains(column: impl Into<SimpleExpr>, key: &str, value: serde_json::Value) -> Condition;
```

### 3.9 Validation rules per type

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
| MultiSelect | Array\<String\>  | array items count    | —       | ✅      |
| Color       | String (#RRGGBB) | —                    | —       | —       |
| Json        | Any              | —                    | —       | —       |

### 3.10 Exports в lib.rs

```rust
// rustok-core/src/lib.rs
pub mod field_schema;

// pub use:
pub use field_schema::{
    CustomFieldsSchema, FieldDefinition, FieldErrorCode, FieldType,
    FieldValidationError, HasCustomFields, SelectOption, ValidationRule,
    create_field_definitions_table, drop_field_definitions_table,
};

// prelude:
pub use crate::field_schema::{
    FieldType, FieldDefinition, HasCustomFields, CustomFieldsSchema,
};
```

### 3.11 Unit Tests

```
validate_required_field_missing
validate_required_field_present
validate_text_min_length
validate_text_max_length
validate_text_pattern_match
validate_text_pattern_mismatch
validate_integer_in_range
validate_integer_below_minimum
validate_integer_above_maximum
validate_decimal_precision
validate_select_valid_option
validate_select_invalid_option
validate_multiselect_valid
validate_multiselect_invalid_option
validate_multiselect_too_many
validate_email_valid
validate_email_invalid
validate_url_valid
validate_url_invalid
validate_date_iso8601_valid
validate_date_iso8601_invalid
validate_datetime_valid
validate_color_hex_valid
validate_color_hex_invalid
validate_boolean_type_mismatch
validate_phone_with_pattern
validate_json_accepts_anything
apply_defaults_fills_missing
apply_defaults_preserves_existing
strip_unknown_removes_extra_keys
strip_unknown_keeps_defined
empty_schema_accepts_anything
field_type_requires_options
```

---

## 4. Part 2 — Модуль подключает Flex (на примере Users)

### 4.1 Шаг 1: Миграция (в apps/server)

```rust
// apps/server/migration/src/m20260315_000001_create_user_field_definitions.rs

use sea_orm_migration::prelude::*;
use rustok_core::field_schema::create_field_definitions_table;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Одна строка — и таблица user_field_definitions готова!
        create_field_definitions_table(manager, "user", "users").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_field_definitions_table(manager, "user").await
    }
}
```

Результат: таблица `user_field_definitions` с полной структурой.

### 4.2 Шаг 2: SeaORM Entity

```rust
// apps/server/src/models/_entities/user_field_definitions.rs
// Либо через макрос:
rustok_core::define_field_definitions_entity!("user_field_definitions");

// Либо вручную (стандартный SeaORM entity) — структура идентична
// для всех модулей, отличается только имя таблицы.
```

### 4.3 Шаг 3: HasCustomFields для User

```rust
// apps/server/src/models/users.rs
use rustok_core::field_schema::HasCustomFields;

impl HasCustomFields for Model {
    fn entity_type() -> &'static str { "user" }

    fn metadata(&self) -> &serde_json::Value {
        &self.metadata
    }

    fn set_metadata(&mut self, value: serde_json::Value) {
        self.metadata = sea_orm::JsonValue::from(value);
    }
}
```

### 4.4 Шаг 4: Service (в apps/server)

```rust
// apps/server/src/services/user_field_service.rs
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition};

pub struct UserFieldService;

impl UserFieldService {
    /// Load schema from user_field_definitions table.
    pub async fn get_schema(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<CustomFieldsSchema> {
        let rows = user_field_definitions::Entity::find()
            .filter(user_field_definitions::Column::TenantId.eq(tenant_id))
            .filter(user_field_definitions::Column::IsActive.eq(true))
            .order_by_asc(user_field_definitions::Column::Position)
            .all(db)
            .await?;

        let definitions: Vec<FieldDefinition> = rows.into_iter()
            .map(|r| r.into_field_definition())  // conversion from DB row to DTO
            .collect();

        Ok(CustomFieldsSchema::new(definitions))
    }

    // CRUD methods: create, update, deactivate, reorder...
    // Same pattern for every module — could be extracted into
    // a generic FieldDefinitionCrud<E: EntityTrait> helper in core.
}
```

### 4.5 Шаг 5: Validation в мутациях

```rust
// В create_user mutation:
let schema = UserFieldService::get_schema(db, tenant_id).await?;
let mut metadata = input.custom_fields.unwrap_or(json!({}));
schema.apply_defaults(&mut metadata);
let errors = schema.validate(&metadata);
if !errors.is_empty() {
    return Err(custom_field_validation_error(errors));
}
// Save user with metadata
```

### 4.6 Шаг 6: GraphQL расширение

```graphql
# Добавить в User:
type User {
    # ... existing ...
    customFields: JSON
    fieldDefinitions: [FieldDefinition!]!
}

input CreateUserInput {
    # ... existing ...
    customFields: JSON
}

# FieldDefinition type — один на всю платформу:
type FieldDefinition {
    id: UUID!
    fieldKey: String!
    fieldType: String!
    label: JSON!
    description: JSON
    isRequired: Boolean!
    defaultValue: JSON
    validation: JSON
    position: Int!
    isActive: Boolean!
}
```

---

## 5. Паттерн для других модулей

Когда `rustok-commerce` захочет кастомные поля для products:

```rust
// 1. Миграция (в rustok-commerce или apps/server/migration):
create_field_definitions_table(manager, "product", "products").await?;

// 2. Entity:
rustok_core::define_field_definitions_entity!("product_field_definitions");

// 3. Trait:
impl HasCustomFields for product::Model {
    fn entity_type() -> &'static str { "product" }
    fn metadata(&self) -> &serde_json::Value { &self.metadata }
    fn set_metadata(&mut self, value: serde_json::Value) { self.metadata = value.into(); }
}

// 4. Service — копипаст минимален, т.к. 90% логики в core.
// 5. Validate в мутациях — идентично users.
```

**Каждый модуль = 5 шагов, ~50 строк нового кода.** Всё остальное — в core.

### Таблицы, создаваемые модулями

| Модуль          | Таблица                         | entity_type  | metadata column      |
|-----------------|---------------------------------|--------------|----------------------|
| apps/server     | `user_field_definitions`        | `"user"`     | `users.metadata`     |
| rustok-commerce | `product_field_definitions`     | `"product"`  | `products.metadata`  |
| rustok-commerce | `order_field_definitions`       | `"order"`    | `orders.metadata`    |
| rustok-content  | `node_field_definitions`        | `"node"`     | `nodes.metadata`     |
| rustok-forum    | `topic_field_definitions`       | `"topic"`    | `topics.metadata`    |

Все таблицы **структурно идентичны**, но **физически изолированы** в своём модуле.

---

## 6. Два режима Flex

1. **Attached mode** — кастомные поля к существующим сущностям → **это то, что описано выше**
2. **Standalone mode** — произвольные схемы и записи (лендинги, формы, справочники)

Standalone mode — это **отдельный модуль `rustok-flex`**, который:
- Имеет свои таблицы (`flex_schemas`, `flex_entries`) — это его "domain"
- Использует `FieldType`, `ValidationRule`, `CustomFieldsSchema` из core
- Является **опциональным** — можно не подключать
- НЕ пересекается с attached mode

```
Attached mode (field_schema в core):
  "Дай мне кастомные поля для users" → user_field_definitions + users.metadata

Standalone mode (rustok-flex модуль):
  "Дай мне произвольную сущность 'landing-page'" → flex_schemas + flex_entries
```

Оба используют **одну и ту же катану** — типы и валидацию из `rustok-core::field_schema`.

---

## 7. Admin API (общий для всех модулей)

### 7.1 GraphQL mutations

```graphql
type Mutation {
    # entityType определяет, в какую таблицу пишем:
    # "user" → user_field_definitions
    # "product" → product_field_definitions
    createFieldDefinition(input: CreateFieldDefinitionInput!): FieldDefinition!
    updateFieldDefinition(id: UUID!, input: UpdateFieldDefinitionInput!): FieldDefinition!
    deleteFieldDefinition(entityType: String, id: UUID!): Boolean!
    reorderFieldDefinitions(entityType: String, ids: [UUID!]!): [FieldDefinition!]!
}

type Query {
    fieldDefinitions(entityType: String): [FieldDefinition!]!
    fieldDefinition(entityType: String, id: UUID!): FieldDefinition
}

input CreateFieldDefinitionInput {
    entityType: String # optional for backward-compatibility (defaults to "user")
    # формат: snake_case, ^[a-z][a-z0-9_]{0,127}$
    fieldKey: String!
    fieldType: String!
    label: JSON!
    description: JSON
    isRequired: Boolean
    defaultValue: JSON
    validation: JSON
    position: Int
}

input UpdateFieldDefinitionInput {
    entityType: String # optional for backward-compatibility (defaults to "user")
    # формат: snake_case, ^[a-z][a-z0-9_]{0,127}$
    label: JSON
    description: JSON
    isRequired: Boolean
    defaultValue: JSON
    validation: JSON
    position: Int
    isActive: Boolean
}
```

### 7.2 Routing по entityType

```rust
// В GraphQL resolver:
fn resolve_table(entity_type: &str) -> Result<Box<dyn FieldDefinitionRepository>> {
    match entity_type {
        "user" => Ok(Box::new(UserFieldRepo)),
        "product" => Ok(Box::new(ProductFieldRepo)),
        "node" => Ok(Box::new(NodeFieldRepo)),
        _ => Err(Error::UnknownEntityType(entity_type)),
    }
}
```

Или через trait object / registry pattern — модули регистрируют свои entity_types при старте.

**Требование к реализации registry:** сам Flex-layer не зависит от конкретных модулей (`users`, `products`, ...).
Регистрация адаптеров выполняется в composition root (server startup / module bootstrap), чтобы при росте до 100+ модулей core-контракт оставался неизменным.

### 7.3 RBAC

- Управление определениями: `Admin`, `SuperAdmin`
- Заполнение кастомных полей: по правам на entity (edit user → edit user custom fields)

### 7.4 Кеширование

```rust
/// Per (tenant_id, entity_type) cache with TTL invalidation.
/// Invalidated on any definition CRUD operation.
type SchemaCache = DashMap<(Uuid, &'static str), (Instant, CustomFieldsSchema)>;
```

---

## 8. Prerequisites — Metadata Readiness

Перед подключением Flex к модулю, сущность **должна** иметь `metadata: Json` колонку.

| Entity   | metadata | Миграция | Статус |
|----------|----------|----------|--------|
| users    | ✅ Есть  | m20250101_000002 | Готово |
| products | ✅ Есть  | m20250130_000012 | Готово |
| product_images | ✅ Есть | m20250130_000012 | Готово |
| nodes    | ✅ Есть  | m20250130_000005 | Готово |
| tenants  | ✅ Есть  | m20250101_000006 | Готово |
| orders   | ❌ Нет   | Таблица orders не найдена | **Нужна миграция** |
| topics   | ⚠️ ?     | Миграции в crates/rustok-forum/ (децентрализованные) | **Проверить** |

**Действия перед Phase 4:**
- [ ] Создать миграцию для `orders` таблицы с metadata колонкой (или добавить ALTER TABLE)
- [ ] Проверить `crates/rustok-forum/` — есть ли metadata в topics
- [ ] Если нет — добавить миграцию в крейте forum

**Важно:** Forum использует **децентрализованные миграции** (в крейте, не в apps/server).
Migration helper `create_field_definitions_table()` должен работать и в таком контексте.

---

## 9. Events Integration

Система events в RusToK **зрелая** (DomainEvent, EventBus, EventDispatcher, backpressure).
Flex CRUD операции **обязаны** эмитить события.

### 9.1 Новые варианты DomainEvent

```rust
// Добавить в rustok-core/src/events/types.rs (или rustok-events crate):

FieldDefinitionCreated {
    tenant_id: Uuid,
    entity_type: String,        // "user", "product", etc.
    field_key: String,
    field_type: String,
},
FieldDefinitionUpdated {
    tenant_id: Uuid,
    entity_type: String,
    field_key: String,
},
FieldDefinitionDeleted {
    tenant_id: Uuid,
    entity_type: String,
    field_key: String,
},
```

### 9.2 Event Consumers

```rust
// 1. Кеш-инвалидация
FieldDefinitionCreated | FieldDefinitionUpdated | FieldDefinitionDeleted => {
    schema_cache.invalidate((tenant_id, entity_type));
}

// 2. Аудит (через существующий AuditLogger)
FieldDefinition* => {
    audit_logger.log(AuditEventType::FieldDefinitionChanged, ...);
}
```

### 9.3 Cascade Policy

- Удаление entity (user, product) → metadata удаляется вместе (CASCADE на уровне row)
- Удаление field definition (soft: is_active=false) → **данные в metadata не трогаем**
- Hard delete field definition → опционально: strip_unknown() при следующем write

---

## 10. Guardrails

| Guardrail | Значение | Где проверяется |
|-----------|----------|-----------------|
| Max fields per entity type per tenant | **50** | `create_definition()` — count before insert |
| Max nesting depth (JSON in metadata) | **2** | `validate_field_value()` для FieldType::Json |
| Validation on write | **Строгая** | `CustomFieldsSchema::validate()` |
| Mandatory pagination | **Да** | GraphQL: использовать `PaginationInput` из `graphql/common.rs` |
| Timeout for JSONB operations | **5s** | Database query timeout |
| field_key format | **snake_case** | Regex: `^[a-z][a-z0-9_]{0,127}$` |
| Locale keys in label | **BCP 47** | Validate keys match `^[a-z]{2}(-[A-Z]{2})?$` |

### 10.1 Реализация в коде

```rust
// В CustomFieldDefinitionService::create():
const MAX_FIELDS_PER_ENTITY: usize = 50;

let count = Entity::find()
    .filter(Column::TenantId.eq(tenant_id))
    .filter(Column::EntityType.eq(entity_type))
    .count(db)
    .await?;

if count >= MAX_FIELDS_PER_ENTITY as u64 {
    return Err(FlexError::TooManyFields {
        entity_type: entity_type.to_string(),
        max: MAX_FIELDS_PER_ENTITY,
    });
}

// Validate field_key format
let key_regex = Regex::new(r"^[a-z][a-z0-9_]{0,127}$").unwrap();
if !key_regex.is_match(&input.field_key) {
    return Err(FlexError::InvalidFieldKey(input.field_key));
}
```

---

## 11. RBAC

### 11.1 Управление определениями

Для field definitions **НЕ добавляем новый `Resource` variant** в permissions.rs.
Используем прямую проверку роли, т.к. field definitions — настройка тенанта:

```rust
// В GraphQL resolver:
fn require_admin(ctx: &Context<'_>) -> Result<()> {
    let claims = ctx.data::<Claims>()?;
    if !matches!(claims.role, UserRole::Admin | UserRole::SuperAdmin) {
        return Err(Error::Forbidden);
    }
    Ok(())
}
```

| Действие | Роли |
|----------|------|
| Просмотр определений | Admin, SuperAdmin |
| Создание/обновление | Admin, SuperAdmin |
| Удаление (soft) | SuperAdmin |

### 11.2 Заполнение кастомных полей

По правам на entity: кто может edit user → может edit custom fields этого user.
Используется существующий RBAC: `Resource::Users` + `Action::Update`.

---

## 12. Entity Type Registry

`ModuleRegistry` НЕ подходит для маршрутизации по entity_type (он про модули, не entity).
Нужен отдельный registry:

```rust
/// Trait для репозитория field definitions конкретного entity type.
#[async_trait]
pub trait FieldDefinitionRepository: Send + Sync {
    fn entity_type(&self) -> &'static str;

    async fn list(
        &self, db: &DatabaseConnection, tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionRow>>;

    async fn create(
        &self, db: &DatabaseConnection, tenant_id: Uuid,
        input: CreateFieldDefinitionInput,
    ) -> Result<FieldDefinitionRow>;

    async fn update(
        &self, db: &DatabaseConnection, tenant_id: Uuid,
        id: Uuid, input: UpdateFieldDefinitionInput,
    ) -> Result<FieldDefinitionRow>;

    async fn deactivate(
        &self, db: &DatabaseConnection, tenant_id: Uuid, id: Uuid,
    ) -> Result<()>;
}

/// Registry: модули регистрируют свои repos при старте приложения.
pub struct FieldDefRegistry {
    repos: HashMap<&'static str, Box<dyn FieldDefinitionRepository>>,
}

impl FieldDefRegistry {
    pub fn register(&mut self, repo: Box<dyn FieldDefinitionRepository>) {
        self.repos.insert(repo.entity_type(), repo);
    }

    pub fn get(&self, entity_type: &str) -> Result<&dyn FieldDefinitionRepository> {
        self.repos.get(entity_type)
            .map(|r| r.as_ref())
            .ok_or(FlexError::UnknownEntityType(entity_type.to_string()))
    }
}
```

Каждый модуль регистрирует свой repo в `after_routes()` или при инициализации:

```rust
// В apps/server при старте:
let mut field_registry = FieldDefRegistry::new();
field_registry.register(Box::new(UserFieldRepo));
// commerce регистрирует:
field_registry.register(Box::new(ProductFieldRepo));
```

---

## 13. Error Handling

Следовать паттерну из `apps/server/src/graphql/` — использовать `ErrorExtensions`:

```rust
use async_graphql::ErrorExtensions;

#[derive(Debug, thiserror::Error)]
pub enum FlexError {
    #[error("Unknown entity type: {0}")]
    UnknownEntityType(String),

    #[error("Too many field definitions for {entity_type} (max {max})")]
    TooManyFields { entity_type: String, max: usize },

    #[error("Invalid field key: {0}")]
    InvalidFieldKey(String),

    #[error("Field key already exists: {0}")]
    DuplicateFieldKey(String),

    #[error("Field definition not found: {0}")]
    NotFound(Uuid),

    #[error("Validation failed")]
    ValidationFailed(Vec<FieldValidationError>),

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

impl ErrorExtensions for FlexError {
    fn extend(&self) -> async_graphql::Error {
        async_graphql::Error::new(self.to_string())
            .extend_with(|_, e| {
                match self {
                    Self::ValidationFailed(errors) => {
                        e.set("code", "VALIDATION_FAILED");
                        e.set("fields", serde_json::to_value(errors).unwrap());
                    }
                    Self::UnknownEntityType(_) => e.set("code", "UNKNOWN_ENTITY_TYPE"),
                    Self::TooManyFields { .. } => e.set("code", "TOO_MANY_FIELDS"),
                    _ => e.set("code", "FLEX_ERROR"),
                }
            })
    }
}
```

---

## 14. Caching

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

const SCHEMA_CACHE_TTL: Duration = Duration::from_secs(300); // 5 min safety net

/// Per (tenant_id, entity_type) schema cache.
/// Primary invalidation: via FieldDefinition* events.
/// Secondary: TTL expiry as safety net.
pub struct SchemaCache {
    inner: DashMap<(Uuid, String), (Instant, CustomFieldsSchema)>,
}

impl SchemaCache {
    pub fn get(&self, tenant_id: Uuid, entity_type: &str) -> Option<CustomFieldsSchema> {
        let key = (tenant_id, entity_type.to_string());
        self.inner.get(&key).and_then(|entry| {
            if entry.0.elapsed() < SCHEMA_CACHE_TTL {
                Some(entry.1.clone())
            } else {
                None
            }
        })
    }

    pub fn invalidate(&self, tenant_id: Uuid, entity_type: &str) {
        self.inner.remove(&(tenant_id, entity_type.to_string()));
    }
}
```

---

## 15. Delivery Phases

### Phase 0 — Core types & validation ⭐ START HERE
- [x] Создать `rustok-core/src/field_schema.rs`
- [x] `FieldType` enum с helper methods (`requires_options()`, `supports_pattern()`)
- [x] `ValidationRule`, `SelectOption`
- [x] `FieldDefinition` (portable DTO)
- [x] `HasCustomFields` trait
- [x] `CustomFieldsSchema` с `validate()`, `apply_defaults()`, `strip_unknown()`
- [x] `FieldValidationError`, `FieldErrorCode`
- [x] `validate_field_value()` — внутренняя функция, все 14 типов
- [x] Guardrails: field_key regex validation, locale key validation
- [x] Unit-тесты (30+ test cases, см. §3.11)
- [x] Exports в `lib.rs` и `prelude`

### Phase 1 — Migration helper + infrastructure
- [x] `create_field_definitions_table()` helper (с tenant FK, indexes)
- [x] `drop_field_definitions_table()` helper
- [ ] `define_field_definitions_entity!()` macro (опционально)
- [x] JSONB query helpers (`json_field_eq`, `json_field_exists`, `json_field_extract`, `json_field_contains`)
- [x] `FlexError` enum с `ErrorExtensions` (§13) *(реализован `map_flex_error()` в GraphQL Flex резолверах с кодами `BAD_USER_INPUT`/`NOT_FOUND`/`INTERNAL_ERROR`)*
- [x] `FieldDefinitionRepository` trait (§12) *(реализован как transport-agnostic `FieldDefinitionService` с CRUD/reorder + registry bootstrap)*
- [x] `FieldDefRegistry` (§12)
- [x] DomainEvent variants: `FieldDefinitionCreated/Updated/Deleted` (§9)
- [x] Integration test: создать/удалить таблицу helper-ом (`sqlite::memory:`) и проверить наличие `user_field_definitions`; сценарий запись+валидация остаётся pending

### Phase 2 — Users (первый потребитель)
- [x] Миграция `user_field_definitions` (через helper — одна строка!)
- [x] SeaORM entity
- [x] `impl HasCustomFields for User`
- [x] `UserFieldService` + регистрация в `FieldDefRegistry`
- [x] Guardrail: max 50 fields per tenant (§10)
- [x] Validation flow в create/update user мутациях
- [x] Event emission: FieldDefinitionCreated/Updated/Deleted
- [x] GraphQL: `customFields` в User type, `fieldDefinitions` resolver
- [x] Error handling через `ErrorExtensions` (§13)
- [ ] Тесты: CRUD, validation, guardrails, events *(добавлены unit-тесты `UserFieldService` для guardrails, events create/update/delete, not-found веток update/deactivate и skip unknown field_type при schema load; интеграционные CRUD/validation сценарии pending)*

### Phase 3 — Admin API
- [x] GraphQL queries/mutations для управления определениями (§7)
- [x] Routing по entityType через `FieldDefRegistry` (§12) *(apps/server GraphQL: `fieldDefinitions`, `fieldDefinition`, `createFieldDefinition`, `updateFieldDefinition`, `deleteFieldDefinition`, `reorderFieldDefinitions`)*
- [x] RBAC: role check Admin/SuperAdmin (§11)
- [x] `SchemaCache` с event-driven invalidation (§14) *(Moka cache + invalidation на мутациях + invalidation listener на `FieldDefinition*` событиях EventBus)*
- [x] Pagination через существующий `PaginationInput` (cursor-based) *(реализовано в `fieldDefinitions` query)*
- [ ] Тесты: RBAC, cache invalidation, pagination *(unit: RBAC + pagination + cache есть; интеграционные GraphQL/cache-invalidation сценарии pending)*

### Phase 4 — Commerce, Content, Forum
- [ ] **Pre-req:** добавить `metadata` колонку в `orders` таблицу (§8)
- [ ] **Pre-req:** проверить `topics.metadata` в crates/rustok-forum/ (§8)
- [ ] `product_field_definitions` (через helper)
- [ ] `order_field_definitions` (через helper, после миграции)
- [ ] `node_field_definitions` (через helper)
- [ ] `topic_field_definitions` (через helper, после проверки)
- [ ] Каждый модуль: 5 шагов, ~50 строк + регистрация в FieldDefRegistry

### Phase 5 — Flex standalone (rustok-flex крейт, future)
- [ ] `flex_schemas` + `flex_entries` — свои таблицы
- [ ] Использует `FieldType`, `ValidationRule`, `CustomFieldsSchema` из core
- [ ] Standalone CRUD (лендинги, формы, справочники)
- [ ] REST + GraphQL APIs
- [ ] Events: `FlexSchemaCreated/Updated/Deleted`, `FlexEntryCreated/Updated/Deleted`
- [ ] RBAC permissions: `flex.schemas.*`, `flex.entries.*`
- [ ] Indexer: `index_flex_entries` denormalized table
- [ ] Подробная спецификация при старте этой фазы

### Phase 6 — Advanced (future)
- [ ] Conditional fields (show B if A = X)
- [ ] Computed fields
- [ ] Field groups (UI sections)
- [ ] Import/export schemas between tenants
- [ ] Full-text search по custom fields через rustok-index
- [ ] Schema versioning (история изменений определений)
- [ ] Data migration tool (ретро-валидация существующих metadata)

---

## 16. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| JSONB performance | GIN index на metadata, лимит 50 полей per entity |
| Schema-data inconsistency | Validate on write; CLI tool для retro-validation (Phase 6) |
| Too many tables | Convention-based naming, helper ensures consistency |
| Breaking changes in FieldType | `#[serde(other)]` Unknown variant for forward compat |
| Macro complexity | Macro is optional — manual entity always works |
| N+1 schema loads | SchemaCache per (tenant, entity_type) + event invalidation |
| orders missing metadata | Миграция в Phase 4 (pre-req) |
| Forum decentralized migrations | Verify helper works in crate context |
| Cache stale after definition change | Event-driven invalidation + TTL safety net |

---

## 17. Tracking and Updates

When updating field schema architecture:

1. Update this file first.
2. Ensure all migration helpers produce identical table structures.
3. Run `cargo test -p rustok-core` — field_schema tests must pass.
