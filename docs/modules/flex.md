# Flex Specification

> **Статус:** Draft v2 (обновлено 2026-03-14)
> **Предыдущая версия:** Concept от 2026-02-06 (полностью пересмотрена)

---

## 1. Обзор

### 1.1 Что такое Flex?

**Flex** — набор типов, валидаторов и migration-хелперов в `rustok-core`, позволяющий
любому модулю добавить кастомные поля за ~50 строк кода. Flex — это **библиотека-катана**,
а не отдельный модуль.

Flex **не имеет своих таблиц, данных или состояния**. Таблицы создаются внутри
модуля-потребителя через convention-based хелперы.

### 1.2 Два режима

| Режим | Что делает | Где живёт |
|-------|------------|-----------|
| **Attached** | Кастомные поля для существующих сущностей (users, products, nodes) | `rustok-core/src/field_schema.rs` + таблицы в модулях |
| **Standalone** | Произвольные схемы и записи (лендинги, формы, справочники) | `rustok-flex` крейт (опциональный, future) |

### 1.3 Происхождение

Идея Flex появилась в ходе обсуждения архитектуры RusToK. Основной вопрос был: "Как
добавить гибкость системе (как Strapi/Directus), но не превратить её в помойку JSONB и
не потерять производительность?"

**Решение:** Flex — это инструмент в core. Каждый модуль сам решает, подключать ли
кастомные поля. Данные остаются внутри модуля. Один набор типов и валидации для всех.

---

## 2. Purpose & Non-Goals

### 2.1 Purpose (для чего)

✅ Runtime-определяемые кастомные поля для любой сущности
✅ Хранение значений в JSONB `metadata` колонке (уже есть в сущностях)
✅ Schema-first валидация при записи
✅ Per-tenant настройка полей (разные тенанты — разные поля)
✅ Локализованные labels (`{"en": "Phone", "ru": "Телефон"}`)
✅ Convention-based migration helper — одна строка кода

### 2.2 Non-Goals (для чего НЕ)

❌ Замена стандартных модулей (content, commerce, blog)
❌ Хранение критических данных (заказы, платежи, инвентарь)
❌ Создание сложных реляционных связей
❌ Альтернатива нормализованным таблицам
❌ Standalone произвольные сущности (это future — standalone mode)

**Правило:** Если кейс закрывается стандартным полем модуля — не используй Flex.

---

## 3. Architectural Rules (HARD LAWS)

> Эти правила **не подлежат обсуждению**.

| # | Правило | Обоснование |
|---|---------|-------------|
| **1** | **Flex — часть core, не отдельный модуль** | Модули зависят только от core |
| **2** | **Данные остаются в модуле** | Таблицы и metadata в модуле-потребителе |
| **3** | **Removal-safe** | Удаление field_schema.rs не ломает платформу (теряются custom fields) |
| **4** | **No Flex in critical domains** | Orders/payments/inventory — через нормализованные поля |
| **5** | **Schema-first** | Все данные валидируются по схеме при записи |
| **6** | **Tenant isolation** | Определения полей per-tenant |

### 3.1 Dependency Diagram

```text
rustok-core
├── src/field_schema.rs    ← типы, валидация, migration helper
│
├── apps/server            ← impl HasCustomFields for User
│   └── user_field_definitions (таблица через helper)
│
├── rustok-commerce        ← impl HasCustomFields for Product
│   └── product_field_definitions (таблица через helper)
│
├── rustok-content         ← impl HasCustomFields for Node
│   └── node_field_definitions (таблица через helper)
│
└── rustok-flex (future, OPTIONAL)
    ├── flex_schemas        ← standalone schemas
    └── flex_entries        ← standalone entries
```

---

## 4. Use Cases

### 4.1 Когда использовать Flex

| Сценарий | Пример |
|----------|--------|
| Кастомные поля к пользователям | Телефон, компания, должность, аватар |
| Кастомные поля к товарам | Специфические характеристики для ниши |
| Расширение любой сущности | Доп. данные без изменения миграций |
| Per-tenant настройка | Каждый тенант настраивает свой набор полей |

### 4.2 Когда НЕ использовать Flex

| Сценарий | Что использовать |
|----------|------------------|
| Стандартные поля (title, price) | Нормализованные колонки модуля |
| Критические данные (платежи) | Типизированные поля в модуле |
| Standalone формы/лендинги | `rustok-flex` standalone mode (future) |
| Сложные связи между сущностями | Нормализованные таблицы + FK |

---

## 5. Core Types (`rustok-core/src/field_schema.rs`)

### 5.1 FieldType

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,           // Однострочный текст
    Textarea,       // Многострочный текст
    Integer,        // Целое число (i64)
    Decimal,        // Число с плавающей точкой (f64)
    Boolean,        // true/false
    Date,           // ISO 8601 (YYYY-MM-DD)
    DateTime,       // ISO 8601 с временем
    Url,            // Валидация формата URL
    Email,          // Валидация формата email
    Phone,          // Свободный формат + optional regex
    Select,         // Выбор одного из options
    MultiSelect,    // Выбор нескольких из options
    Color,          // #RRGGBB
    Json,           // Произвольная JSON-структура
}
```

### 5.2 ValidationRule

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationRule {
    pub min: Option<f64>,                           // Длина строки / значение числа
    pub max: Option<f64>,                           // Длина строки / значение числа
    pub pattern: Option<String>,                    // Regex (для Text, Textarea, Phone)
    pub options: Option<Vec<SelectOption>>,          // Для Select/MultiSelect
    pub error_message: Option<HashMap<String, String>>, // Локализованная ошибка
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: HashMap<String, String>,             // {"en": "Male", "ru": "Мужской"}
}
```

### 5.3 FieldDefinition (portable DTO)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub field_key: String,                          // snake_case, e.g. "phone_number"
    pub field_type: FieldType,
    pub label: HashMap<String, String>,             // {"en": "Phone", "ru": "Телефон"}
    pub description: Option<HashMap<String, String>>,
    pub is_required: bool,
    pub default_value: Option<serde_json::Value>,
    pub validation: Option<ValidationRule>,
    pub position: i32,
    pub is_active: bool,
}
```

### 5.4 HasCustomFields trait

```rust
pub trait HasCustomFields {
    fn entity_type() -> &'static str;               // "user", "product", "node"
    fn metadata(&self) -> &serde_json::Value;
    fn set_metadata(&mut self, value: serde_json::Value);
}
```

### 5.5 CustomFieldsSchema (валидатор)

```rust
pub struct CustomFieldsSchema { definitions: Vec<FieldDefinition> }

impl CustomFieldsSchema {
    pub fn new(definitions: Vec<FieldDefinition>) -> Self;
    pub fn validate(&self, metadata: &serde_json::Value) -> Vec<FieldValidationError>;
    pub fn apply_defaults(&self, metadata: &mut serde_json::Value);
    pub fn strip_unknown(&self, metadata: &mut serde_json::Value);
    pub fn active_definitions(&self) -> Vec<&FieldDefinition>;
}
```

### 5.6 Migration Helper

```rust
/// Одна строка — и модуль получает таблицу кастомных полей.
pub async fn create_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,           // "user" → "user_field_definitions"
    _parent_table: &str,    // "users"
) -> Result<(), DbErr>;

pub async fn drop_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,
) -> Result<(), DbErr>;
```

---

## 6. Data Model

### 6.1 `{entity}_field_definitions` (создаётся через helper)

Каждый модуль создаёт **свою таблицу** через `create_field_definitions_table()`.
Структура идентична для всех:

| Column          | Type           | Notes                                    |
|-----------------|----------------|------------------------------------------|
| `id`            | UUID PK        |                                          |
| `tenant_id`     | UUID FK        | → tenants.id, ON DELETE CASCADE          |
| `field_key`     | String(128)    | snake_case, UNIQUE per tenant            |
| `field_type`    | String(32)     | FieldType as string                      |
| `label`         | JSONB NOT NULL | Локализованные labels                    |
| `description`   | JSONB nullable | Локализованное описание                  |
| `is_required`   | Boolean        | default false                            |
| `default_value` | JSONB nullable |                                          |
| `validation`    | JSONB nullable | ValidationRule serialized                |
| `position`      | i32            | Порядок отображения, default 0           |
| `is_active`     | Boolean        | default true (soft-disable)              |
| `created_at`    | TimestampTZ    |                                          |
| `updated_at`    | TimestampTZ    |                                          |

**Indexes:**
- `UNIQUE (tenant_id, field_key)` — одно определение на ключ per tenant
- `idx_{prefix}_fd_tenant_active (tenant_id, is_active)` — lookup

### 6.2 Хранение значений

Значения хранятся в **существующих** `metadata: Json` колонках сущностей:

```json
{
  "phone_number": "+7 999 123-45-67",
  "company": "Acme Corp",
  "interests": ["rust", "cms"],
  "agreed_to_newsletter": true
}
```

### 6.3 Текущее состояние metadata колонок

| Entity   | metadata exists? | Статус |
|----------|-----------------|--------|
| users    | ✅ Есть         | Готово |
| products | ✅ Есть         | Готово |
| nodes    | ✅ Есть         | Готово |
| tenants  | ✅ Есть         | Готово |
| orders   | ❌ Нет          | Нужна миграция |
| topics   | ⚠️ В крейте     | Проверить crates/rustok-forum |

---

## 7. Guardrails (обязательные ограничения)

| Guardrail | Значение | Обоснование |
|-----------|----------|-------------|
| Max fields per entity type | **50** | JSONB performance, UI complexity |
| Max nesting depth | **2** | Предотвращает сложные вложенные структуры |
| Validation on write | **Строгая** | CustomFieldsSchema::validate() при каждом write |
| Mandatory pagination | **Да** | Cursor-based (PaginationInput из graphql/common.rs) |
| Timeout for operations | **5s** | Защита от тяжёлых JSONB-запросов |
| Locale validation | **BCP 47** | Ключи в label/description — ISO 639-1 или BCP 47 |
| field_key format | **snake_case** | Regex: `^[a-z][a-z0-9_]{0,127}$` |

---

## 8. Events

### 8.1 Emitted Events

Каждая CRUD-операция на field definitions эмитит события через существующую
систему `DomainEvent` + `EventBus`:

```rust
// В rustok-core/src/events/types.rs — добавить варианты:

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

### 8.2 Event Consumers

```rust
// Кеш-инвалидация: при изменении определения → очистить SchemaCache
FieldDefinitionCreated | FieldDefinitionUpdated | FieldDefinitionDeleted => {
    schema_cache.invalidate((tenant_id, entity_type));
}

// Аудит: все изменения определений логируются
FieldDefinitionCreated | ... => {
    audit_logger.log(AuditEventType::FieldDefinitionChanged, ...);
}
```

### 8.3 Cascade Policy

Когда удаляется сущность (user, product), её metadata удаляется вместе с ней
(CASCADE на уровне строки). Field definitions **не удаляются** при удалении
конкретной сущности — они привязаны к tenant, не к record.

---

## 9. API

### 9.1 GraphQL — Field Definitions (Admin API)

```graphql
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
    createdAt: String!
    updatedAt: String!
}

type Query {
    fieldDefinitions(entityType: String!): [FieldDefinition!]!
    fieldDefinition(id: UUID!): FieldDefinition
}

type Mutation {
    createFieldDefinition(input: CreateFieldDefinitionInput!): FieldDefinition!
    updateFieldDefinition(id: UUID!, input: UpdateFieldDefinitionInput!): FieldDefinition!
    deleteFieldDefinition(id: UUID!): Boolean!
    reorderFieldDefinitions(entityType: String!, ids: [UUID!]!): [FieldDefinition!]!
}
```

### 9.2 GraphQL — Entity Integration

```graphql
# Добавляется к каждой сущности, подключившей Flex:
type User {
    # ... existing fields ...
    customFields: JSON                              # metadata values
    fieldDefinitions: [FieldDefinition!]!            # schema for this entity
}

input CreateUserInput {
    # ... existing fields ...
    customFields: JSON
}
```

### 9.3 Entity Type Routing

```rust
/// Registry: entity_type → table/repository.
/// Модули регистрируют свои entity types при старте.
pub trait FieldDefinitionRepository: Send + Sync {
    fn entity_type(&self) -> &'static str;
    async fn list(&self, db: &DatabaseConnection, tenant_id: Uuid) -> Result<Vec<Model>>;
    async fn create(&self, db: &DatabaseConnection, tenant_id: Uuid, input: ...) -> Result<Model>;
    // ...
}

/// Dispatch по entityType в GraphQL resolver:
fn resolve_repo(registry: &FieldDefRegistry, entity_type: &str) -> Result<&dyn FieldDefinitionRepository>;
```

### 9.4 REST (future)

REST endpoints не реализуются в первых фазах. При необходимости — по аналогии
с существующими REST контроллерами.

---

## 10. RBAC

### 10.1 Управление определениями

| Действие | Роли |
|----------|------|
| Просмотр определений | Admin, SuperAdmin |
| Создание/обновление определений | Admin, SuperAdmin |
| Удаление определений | SuperAdmin |

### 10.2 Заполнение кастомных полей

По правам на саму entity: кто может edit user → может edit custom fields этого user.

### 10.3 Реализация

Для field definitions **не добавляем новый `Resource` variant** в RBAC.
Используем прямую проверку роли (Admin/SuperAdmin), т.к. field definitions — это
настройка тенанта, аналогичная tenant settings.

---

## 11. Caching

```rust
/// Per (tenant_id, entity_type) cache with TTL.
/// Invalidated on FieldDefinitionCreated/Updated/Deleted events.
/// Safety TTL: 5 minutes.
type SchemaCache = DashMap<(Uuid, &'static str), (Instant, CustomFieldsSchema)>;
```

---

## 12. How to Add Flex to a Module (5 шагов)

```rust
// STEP 1: Migration (одна строка!)
// В migration файле модуля:
create_field_definitions_table(manager, "product", "products").await?;

// STEP 2: SeaORM Entity
rustok_core::define_field_definitions_entity!("product_field_definitions");
// Или вручную — стандартный entity.

// STEP 3: HasCustomFields trait
impl HasCustomFields for product::Model {
    fn entity_type() -> &'static str { "product" }
    fn metadata(&self) -> &serde_json::Value { &self.metadata }
    fn set_metadata(&mut self, value: serde_json::Value) { self.metadata = value.into(); }
}

// STEP 4: Service — загрузка схемы + CRUD (паттерн одинаковый для всех)
let schema = field_service.get_schema(db, tenant_id, "product").await?;

// STEP 5: Validation в мутациях
schema.apply_defaults(&mut metadata);
let errors = schema.validate(&metadata);
```

~50 строк нового кода. Всё остальное — в core.

---

## 13. Standalone Mode (future — `rustok-flex` крейт)

> Этот режим будет реализован позже как **отдельный опциональный крейт**.

Для создания **произвольных сущностей** (лендинги, формы, справочники),
которые не привязаны к стандартным модулям:

- `flex_schemas` + `flex_entries` — собственные таблицы `rustok-flex`
- Переиспользует `FieldType`, `ValidationRule`, `CustomFieldsSchema` из core
- Опциональный модуль, removal-safe
- REST + GraphQL APIs
- Events: `FlexSchemaCreated/Updated/Deleted`, `FlexEntryCreated/Updated/Deleted`
- RBAC: `flex.schemas.read/write/delete`, `flex.entries.read/write/delete`
- Indexer: `index_flex_entries` denormalized table

Подробная спецификация standalone mode будет создана при реализации Phase 5.

---

## 14. Open Questions

1. **Schema versioning:** Нужна ли история изменений определений полей?
2. **Data migration on schema change:** Как мигрировать metadata при удалении/переименовании поля?
3. **Computed fields:** Нужны ли поля, вычисляемые по формуле? (future)
4. **Conditional fields:** Показывать поле B только если поле A = X? (future)
5. **Field groups:** Визуальная группировка полей в UI? (future)

---

## См. также

- [Flex Architecture & Implementation Plan](../architecture/flex.md) — детальный план реализации
- [ARCHITECTURE_GUIDE.md](../ARCHITECTURE_GUIDE.md) — общая архитектура
