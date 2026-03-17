# Flex — Custom Fields System

> **Статус attached mode:** Phases 0–4 реализованы. Phase 4.5 (вынос в `crates/flex`) — в процессе.
> **Статус standalone mode:** Phase 5 — не начат.
> Нереализованное → [`implementation-plan.md`](./implementation-plan.md)

---

## 1. Что такое Flex

**Flex — это катана, а не склад мечей.**

Flex — модуль-библиотека: набор типов, валидаторов и хелперов для миграций внутри `rustok-core`, который позволяет **любому модулю** добавить runtime-определяемые кастомные поля за минимум кода.

Flex существует **рядом** со стандартными модулями, а не **вместо** них. Это «запасной выход» для edge-cases:
- Стандартных доменных модулей (content, commerce, blog) недостаточно
- Создавать отдельный доменный модуль нецелесообразно
- Бизнес хочет кастомные поля без программирования

### Для чего Flex

✅ Кастомные поля к существующим сущностям (attached mode)
✅ Runtime-определяемые схемы данных
✅ Хранение дополнительных данных в JSONB
✅ Маркетинговые лендинги, формы, справочники (standalone mode, Phase 5)

### Для чего Flex НЕ предназначен

❌ Замена стандартных модулей (content, commerce, blog)
❌ Хранение критических данных (заказы, платежи, инвентарь)
❌ Создание сложных реляционных связей
❌ Альтернатива нормализованным таблицам

---

## 2. Архитектурные законы (HARD LAWS)

| # | Правило | Обоснование |
|---|---------|-------------|
| 1 | **Standard modules NEVER depend on Flex** | Flex — опция, не зависимость |
| 2 | **Flex зависит только от `rustok-core`** | Модули зависят только от core |
| 3 | **Removal-safe** | Удалить `field_schema.rs` — платформа работает (теряются custom fields) |
| 4 | **Данные остаются в модуле** | Таблицы и metadata JSONB в модуле-потребителе |
| 5 | **Schema-first** | Все данные валидируются по схеме при записи |
| 6 | **Tenant isolation** | Определения полей per-tenant |
| 7 | **No Flex in critical domains** | Orders/payments/inventory — нормализованные поля |

```text
rustok-core  ←  зависят все
    ↑
field_schema.rs (библиотека типов)

rustok-commerce ←✗→ flex  (НЕТ зависимости от flex!)
rustok-content  ←✗→ flex
```

---

## 3. Два режима

### Attached mode (реализовано, Phases 0–4)

Кастомные поля прикрепляются к существующим сущностям через JSONB-колонку `metadata`:

```
"Дай мне кастомные поля для users"
  → user_field_definitions (таблица определений)
  + users.metadata (JSONB с данными)
```

### Standalone mode (Phase 5, не реализовано)

Произвольные схемы и записи без привязки к существующим сущностям:

```
"Дай мне произвольную сущность 'landing-page'"
  → flex_schemas (определение схемы)
  + flex_entries (записи данных)
```

Оба режима используют одну и ту же библиотеку типов из `rustok-core::field_schema`.

---

## 4. Core types (`rustok-core/src/field_schema.rs`)

### 4.1 FieldType — 14 типов полей

```rust
pub enum FieldType {
    Text,        // Однострочный текст
    Textarea,    // Многострочный текст
    Integer,     // i64
    Decimal,     // f64
    Boolean,     // true/false
    Date,        // ISO 8601 дата (YYYY-MM-DD)
    DateTime,    // ISO 8601 дата-время
    Url,         // URL с проверкой формата
    Email,       // Email с проверкой формата
    Phone,       // Телефон (с опциональным regex)
    Select,      // Один вариант из списка
    MultiSelect, // Несколько вариантов (массив)
    Color,       // Hex-цвет (#RRGGBB)
    Json,        // Произвольный JSON (с guardrail на глубину)
}
```

### 4.2 Правила валидации по типу

| FieldType   | JSON type        | min/max              | pattern | options |
|-------------|------------------|----------------------|---------|---------|
| Text        | String           | длина строки         | ✅      | —       |
| Textarea    | String           | длина строки         | ✅      | —       |
| Integer     | Number (i64)     | числовое значение    | —       | —       |
| Decimal     | Number (f64)     | числовое значение    | —       | —       |
| Boolean     | Boolean          | —                    | —       | —       |
| Date        | String (ISO)     | —                    | —       | —       |
| DateTime    | String (ISO)     | —                    | —       | —       |
| Url         | String           | длина строки         | —       | —       |
| Email       | String           | длина строки         | —       | —       |
| Phone       | String           | длина строки         | ✅      | —       |
| Select      | String           | —                    | —       | ✅      |
| MultiSelect | Array\<String\>  | кол-во элементов     | —       | ✅      |
| Color       | String (#RRGGBB) | —                    | —       | —       |
| Json        | Any              | — (см. глубина)      | —       | —       |

### 4.3 ValidationRule и SelectOption

```rust
pub struct ValidationRule {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub pattern: Option<String>,
    pub options: Option<Vec<SelectOption>>,
    pub error_message: Option<HashMap<String, String>>, // локализованные сообщения
}

pub struct SelectOption {
    pub value: String,
    pub label: HashMap<String, String>, // {"en": "Active", "ru": "Активен"}
}
```

### 4.4 FieldDefinition — переносимый DTO

```rust
pub struct FieldDefinition {
    pub field_key: String,                              // snake_case, уникальный в scope tenant+entity
    pub field_type: FieldType,
    pub label: HashMap<String, String>,                 // {"en": "Phone", "ru": "Телефон"}
    pub description: Option<HashMap<String, String>>,
    pub is_required: bool,
    pub default_value: Option<serde_json::Value>,
    pub validation: Option<ValidationRule>,
    pub position: i32,
    pub is_active: bool,
}
```

### 4.5 CustomFieldsSchema — валидатор

```rust
impl CustomFieldsSchema {
    /// Построить схему из списка определений (из БД, конфига, JSONB)
    pub fn new(definitions: Vec<FieldDefinition>) -> Self;

    /// Валидировать metadata. Пустой список = валидно.
    pub fn validate(&self, metadata: &serde_json::Value) -> Vec<FieldValidationError>;

    /// Заполнить default_value для отсутствующих полей
    pub fn apply_defaults(&self, metadata: &mut serde_json::Value);

    /// Удалить поля, не входящие в схему
    pub fn strip_unknown(&self, metadata: &mut serde_json::Value);

    /// Только активные определения в порядке position
    pub fn active_definitions(&self) -> Vec<&FieldDefinition>;
}
```

### 4.6 HasCustomFields trait

```rust
pub trait HasCustomFields {
    fn entity_type() -> &'static str;          // "user", "product", "node"
    fn metadata(&self) -> &serde_json::Value;
    fn set_metadata(&mut self, value: serde_json::Value);
}
```

### 4.7 Migration helper

```rust
/// Создать таблицу `{prefix}_field_definitions` в миграции любого модуля.
/// Одна строка кода — и модуль получает полноценную таблицу кастомных полей.
pub async fn create_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,       // "user" → создаёт "user_field_definitions"
    _parent_table: &str,
) -> Result<(), DbErr>;

pub async fn drop_field_definitions_table(
    manager: &SchemaManager<'_>,
    prefix: &str,
) -> Result<(), DbErr>;
```

Создаёт таблицу с колонками: `id`, `tenant_id`, `field_key`, `field_type`, `label`,
`description`, `is_required`, `default_value`, `validation`, `position`, `is_active`,
`created_at`, `updated_at`.
Индексы: `UNIQUE(tenant_id, field_key)`, `idx_{prefix}_fd_tenant_active`.

### 4.8 SeaORM entity macro

```rust
/// Генерирует SeaORM entity для таблицы field_definitions одной строкой.
rustok_core::define_field_definitions_entity!("user_field_definitions");
// Генерирует: Entity, Model, ActiveModel, Column, Relation, PrimaryKey
```

### 4.9 JSONB query helpers

```rust
/// metadata->>'key' = 'value'
pub fn json_field_eq(column, key: &str, value: &str) -> Condition;

/// metadata ? 'key'  (ключ существует)
pub fn json_field_exists(column, key: &str) -> Condition;

/// metadata->>'key'  (для ORDER BY)
pub fn json_field_extract(column, key: &str) -> SimpleExpr;

/// metadata @> '{"key": value}'  (contains)
pub fn json_field_contains(column, key: &str, value: serde_json::Value) -> Condition;
```

---

## 5. Guardrails

| Guardrail | Значение | Статус | Где проверяется |
|-----------|----------|--------|-----------------|
| Max fields per entity type per tenant | **50** | ⬜ service-layer TODO | `create_definition()` до вставки |
| Max nesting depth (`FieldType::Json`) | **2** | ✅ реализовано | `validate_field_value()` |
| Validation on write | **Строгая** | ✅ реализовано | `CustomFieldsSchema::validate()` |
| `field_key` format | `^[a-z][a-z0-9_]{0,127}$` | ✅ реализовано | `is_valid_field_key()` |
| Locale key format | BCP 47 short | ✅ реализовано | `is_valid_locale_key()` |
| Mandatory pagination | Да | ✅ реализовано | `fieldDefinitions` GraphQL query |
| Timeout for JSONB operations | 5s | ⬜ TODO | DB query timeout |

### 5.1 Метод счёта глубины JSON — Variant A (массивы прозрачны)

Для `FieldType::Json` считаются только уровни JSON-объектов `{…}`. Массивы `[…]` не создают уровень.

| Значение | Object-depth | Разрешено? |
|----------|-------------|-----------|
| `42` / `"hello"` / `true` / `null` | 0 | ✅ |
| `[1, 2, 3]` | 0 | ✅ |
| `{"key": "value"}` | 1 | ✅ |
| `{"items": [1, 2, 3]}` | 1 | ✅ |
| `{"address": {"city": "NY"}}` | 2 | ✅ (граница) |
| `{"items": [{"id": 1, "name": "x"}]}` | 2 | ✅ (массив прозрачен) |
| `{"a": {"b": {"c": 1}}}` | 3 | ❌ `NestingTooDeep` |

**Почему Variant A:** паттерн `{"items": [{"id":1}]}` встречается в любой CMS постоянно. При счёте массивов он давал бы depth=3 и блокировался без пользы. Variant A при лимите 2 запрещает именно тройную вложенность объектов.

**Реализация:** `json_object_depth()` + `MAX_JSON_NESTING_DEPTH = 2` в `rustok-core/src/field_schema.rs`.
**Ошибка:** `FieldErrorCode::NestingTooDeep` с текущей глубиной и лимитом в message.

---

## 6. Как подключить Flex к модулю (5 шагов)

Каждый модуль = ~50 строк нового кода. Всё остальное — в core.

### Шаг 1: Миграция

```rust
// apps/server/migration/src/m20260315_000001_create_user_field_definitions.rs
use rustok_core::field_schema::create_field_definitions_table;

async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    create_field_definitions_table(manager, "user", "users").await
}
async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    drop_field_definitions_table(manager, "user").await
}
```

### Шаг 2: SeaORM Entity

```rust
rustok_core::define_field_definitions_entity!("user_field_definitions");
```

### Шаг 3: HasCustomFields для сущности

```rust
impl HasCustomFields for user::Model {
    fn entity_type() -> &'static str { "user" }
    fn metadata(&self) -> &serde_json::Value { &self.metadata }
    fn set_metadata(&mut self, value: serde_json::Value) { self.metadata = value.into(); }
}
```

### Шаг 4: Service

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

### Шаг 5: Валидация в мутациях

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

## 7. Текущие потребители (attached mode)

| Модуль | Таблица | entity_type | metadata column |
|--------|---------|-------------|-----------------|
| apps/server | `user_field_definitions` | `"user"` | `users.metadata` |
| apps/server | `product_field_definitions` | `"product"` | `products.metadata` |
| apps/server | `order_field_definitions` | `"order"` | `orders.metadata` |
| apps/server | `node_field_definitions` | `"node"` | `nodes.metadata` |
| apps/server | `topic_field_definitions` | `"topic"` | `nodes.metadata` (kind=topic) |

Все таблицы структурно идентичны, физически изолированы в своём модуле.

---

## 8. Admin API (GraphQL)

### Queries

```graphql
fieldDefinitions(entityType: String): [FieldDefinition!]!
fieldDefinition(entityType: String, id: UUID!): FieldDefinition
```

### Mutations

```graphql
createFieldDefinition(input: CreateFieldDefinitionInput!): FieldDefinition!
updateFieldDefinition(id: UUID!, input: UpdateFieldDefinitionInput!): FieldDefinition!
deleteFieldDefinition(entityType: String, id: UUID!): Boolean!
reorderFieldDefinitions(entityType: String, ids: [UUID!]!): [FieldDefinition!]!
```

### Routing по entityType

Запросы маршрутизируются через `FieldDefRegistry` — модули регистрируют свои репозитории при старте:

```rust
let mut registry = FieldDefRegistry::new();
registry.register(Box::new(UserFieldRepo));
registry.register(Box::new(ProductFieldRepo));
// ...

// В resolver:
let repo = registry.get(entity_type)?; // → FlexError::UnknownEntityType если не найден
```

### RBAC

| Действие | Роли |
|----------|------|
| Просмотр определений | Admin, SuperAdmin |
| Создание / обновление | Admin, SuperAdmin |
| Удаление (soft: is_active=false) | SuperAdmin |

Заполнение кастомных полей — по правам на саму сущность: `Resource::Users + Action::Update`.

---

## 9. События

### Эмитируемые события

```rust
DomainEvent::FieldDefinitionCreated { tenant_id, entity_type, field_key, field_type }
DomainEvent::FieldDefinitionUpdated { tenant_id, entity_type, field_key }
DomainEvent::FieldDefinitionDeleted { tenant_id, entity_type, field_key }
```

### Consumers

```rust
// Инвалидация кеша схемы
FieldDefinitionCreated | FieldDefinitionUpdated | FieldDefinitionDeleted => {
    schema_cache.invalidate(tenant_id, entity_type);
}

// Аудит
FieldDefinition* => {
    audit_logger.log(AuditEventType::ConfigurationChange, ...);
}
```

### Cascade policy

- Удаление entity (user, product) → metadata удаляется вместе (CASCADE на уровне row)
- Soft delete field definition (`is_active=false`) → данные в metadata не трогаем
- Hard delete field definition → `strip_unknown()` при следующей записи

---

## 10. Кеширование схемы

```rust
const SCHEMA_CACHE_TTL: Duration = Duration::from_secs(300); // safety net

/// Per (tenant_id, entity_type) кеш.
/// Основная инвалидация: через события FieldDefinition*.
/// Вторичная: TTL как страховка.
pub struct SchemaCache {
    inner: DashMap<(Uuid, String), (Instant, CustomFieldsSchema)>,
}
```

Реализация: Moka cache + event-driven invalidation на мутациях + listener на `FieldDefinition*` событиях EventBus. В agnostic-слое доступны helper-ы `list_field_definitions_with_cache()` и `invalidate_field_definition_cache()` + порт `FieldDefinitionCachePort`.

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

Все ошибки маппятся через transport-agnostic `flex::map_flex_error()`; в GraphQL выполняется только адаптация в `FieldError` с соответствующими кодами в error extensions.

---

## 12. Standalone mode (Phase 5 — не реализовано)

Спецификация для будущей реализации.

### Data model

```sql
-- Определения произвольных схем
CREATE TABLE flex_schemas (
    id          UUID PRIMARY KEY,
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug        VARCHAR(64) NOT NULL,       -- 'landing-page', 'feedback-form'
    name        VARCHAR(255) NOT NULL,
    fields_config JSONB NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (tenant_id, slug)
);

-- Записи данных
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
```

### Guardrails standalone mode

- Max relation depth = 1 (нет рекурсивного populate)
- FlexEntry A может ссылаться на User/Product ✅
- FlexEntry A → FlexEntry B → FlexEntry C ❌

Подробности реализации — в [`implementation-plan.md`](./implementation-plan.md).

---

## См. также

- [`implementation-plan.md`](./implementation-plan.md) — нереализованное (Phase 4 долги, Phase 4.5, 5, 6)
- [`rustok-core/src/field_schema.rs`](../../crates/rustok-core/src/field_schema.rs) — исходный код core типов
- [`docs/architecture/flex.md`](../../docs/architecture/flex.md) — архитектурный обзор
