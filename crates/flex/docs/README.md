# Flex — Custom Fields System

> **Статус attached mode:** Phases 0–4 реализованы. Phase 4.5 (вынос в `crates/flex`) — в процессе.
> **Статус standalone mode:** Phase 5 — в активной реализации (transport-agnostic контракты, persistence/service layer, GraphQL и REST surfaces уже live; rollout/governance contract уже зафиксирован, полный integration verification остаётся отдельным verification debt).
> **Статус module-system wiring:** Phase 4.6 теперь живая: `flex` занесён в `modules.toml` как `capability_only` ghost module с `rustok-module.toml` и runtime `FlexModule`, но transport-слой остаётся в `apps/server`.
> Нереализованное → [`implementation-plan.md`](./implementation-plan.md)

> **Важно по многоязычности:** для `flex` уже действует общий platform contract. `FieldDefinition.is_localized` является live-частью DB/runtime контракта; standalone schema copy (`name`, `description`) хранится в `flex_schema_translations`; standalone entry values теперь разделены на `flex_entries.data` (shared/non-localized payload) и `flex_entry_localized_values` (locale-aware payload per `entry_id + locale`); attached-mode locale-aware values имеют canonical storage-path в `flex_attached_localized_values`, а shared entity/helpers для этого path живут в `crates/flex`. Live write/read path уже проведён для `user`, `product`, `order` и `topic`; для `topic` donor payload теперь живёт в `forum_topics.metadata`, а locale-aware Flex values идут в parallel attached rows по тому же контракту.
> Cleanup/backfill residual inline locale-aware payload-ов должен выполняться миграциями; runtime path не должен читать donor/base-row inline localized JSON как канонический fallback.

---

## Назначение

`flex` задаёт capability-слой custom fields для RusToK: attached-mode расширяет donor-модули, а standalone mode даёт schema/entry runtime для extension-сценариев без превращения `flex` в отдельный business bounded context.

## Зона ответственности

- transport-agnostic field definition contracts и registry/orchestration helpers;
- multilingual storage/runtime contract для attached и standalone Flex payload;
- capability-only module metadata для `modules.toml` / `rustok-module.toml` / `ModuleRegistry`;
- правила, по которым donor persistence ownership остаётся у модулей-потребителей.

## Интеграция

- `rustok-core::field_schema` поставляет базовые типы, validation rules и migration helpers;
- `crates/flex` держит shared attached/standalone contracts и runtime metadata через `FlexModule`;
- `apps/server` остаётся adapter-слоем для SeaORM, GraphQL, REST, RBAC, bootstrap и event emission;
- donor write/read paths сейчас живые для `user`, `product`, `order` и `topic`.

## Проверка

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`

## Связанные документы

- [`implementation-plan.md`](./implementation-plan.md)
- [`../README.md`](../README.md)
- [`../../../docs/modules/manifest.md`](../../../docs/modules/manifest.md)
- [`../../../docs/architecture/database.md`](../../../docs/architecture/database.md)

---

## 0. Текущая архитектурная граница

Каноническая модульная документация Flex живёт в этом файле.

Текущая архитектура разделена на три слоя:

- `rustok-core::field_schema` хранит базовые типы, валидаторы и migration helpers для attached mode;
- `crates/flex` хранит transport-agnostic orchestration, registry и standalone contracts;
- `apps/server` держит adapter/wiring слой: SeaORM, GraphQL, cache/RBAC gates, event emission и bootstrap.

Attached mode считается рабочим production contract. Standalone mode уже имеет live GraphQL и REST API surfaces в `apps/server`; rollout/governance policy для этого surface теперь тоже зафиксирован, а незакрытым остаётся именно полный integration verification.

`flex` не считается обычным bounded-context модулем. В module-system он живёт как capability-only ghost module: registry/RBAC/runtime metadata формализованы manifest-слоем, но owner'ом donor persistence остаются модули-потребители.

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

Кастомные поля прикрепляются к существующим сущностям через donor payload и, для `is_localized=true`,
через parallel localized records:

```
"Дай мне кастомные поля для users"
  → user_field_definitions (таблица определений)
  + users.metadata (shared / non-localized data)
  + flex_attached_localized_values (locale-aware attached values)
```

### Standalone mode (Phase 5, live GraphQL surface + незавершённый rollout)

Произвольные схемы и записи без привязки к существующим сущностям:

```
"Дай мне произвольную сущность 'landing-page'"
  → flex_schemas (определение схемы)
  + flex_entries (shared/non-localized запись)
  + flex_entry_localized_values (locale-aware payload per entry)
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
    pub is_localized: bool,
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
    fn entity_type() -> &'static str;          // "user", "product", "topic"
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
| Max standalone fields per schema | **50** | ✅ реализовано для standalone | `validate_create_schema_command()` / `validate_update_schema_command()` до adapter/service layer |
| Длина standalone schema slug | **64** символа | ✅ реализовано для standalone | `validate_create_schema_command()` до записи в `flex_schemas.slug VARCHAR(64)` |
| Длина standalone schema name | **255** символов | ✅ реализовано для standalone | create/update validators до записи в `flex_schema_translations.name VARCHAR(255)` |
| Длина standalone entry relation type | **64** символа | ✅ реализовано для standalone | `validate_create_entry_command()` до записи в `flex_entries.entity_type VARCHAR(64)` |
| Формат и длина standalone entry status | `^[a-z][a-z0-9_]*$`, **32** символа | ✅ реализовано для standalone | create/update entry validators до записи в `flex_entries.status VARCHAR(32)` |
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

| Модуль | Таблица | entity_type | donor payload |
|--------|---------|-------------|---------------|
| apps/server | `user_field_definitions` | `"user"` | `users.metadata` + `flex_attached_localized_values` |
| apps/server + `crates/flex` | `product_field_definitions` | `"product"` | `products.metadata` + `flex_attached_localized_values` |
| apps/server + `crates/flex` | `order_field_definitions` | `"order"` | `orders.metadata` + `flex_attached_localized_values` |
| apps/server + `crates/flex` | `topic_field_definitions` | `"topic"` | `forum_topics.metadata` + `flex_attached_localized_values` |

Все таблицы определений структурно идентичны, физически изолированы в своём модуле. Для attached localized values canonical shared storage теперь живёт в `flex_attached_localized_values`, а shared entity/helpers вынесены в `crates/flex`; `user`, `product`, `order` и `topic` уже используют этот path в live read/write flow.

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

| Surface | Typed permissions |
|---------|-------------------|
| Attached field definitions query roots | `flex_schemas:list`, `flex_schemas:read` |
| Attached field definitions mutations | `flex_schemas:create`, `flex_schemas:update`, `flex_schemas:delete` |
| Standalone schema queries/mutations | `flex_schemas:*` |
| Standalone entry queries/mutations | `flex_entries:*` |

Typed permission checks идут через `require_permission(...)` в GraphQL и `RequireFlex*` extractors в REST adapter layer.
Заполнение attached custom fields остаётся привязанным к donor write-path и его собственным правам на сущность.

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

## 12. Standalone mode (Phase 5 — GraphQL + REST live, rollout/governance contract зафиксирован)

На текущем этапе для standalone mode уже live:

- `FlexSchemaView`, `FlexEntryView`
- `CreateFlexSchemaCommand`, `UpdateFlexSchemaCommand`
- `CreateFlexEntryCommand`, `UpdateFlexEntryCommand`
- `FlexStandaloneService`
- Guardrail validators: `validate_create_schema_command`, `validate_update_schema_command`, `validate_create_entry_command`, `validate_update_entry_command` теперь проверяют форму JSON-object для payload, нормализованные identifiers/statuses/schema names, лимит 50 fields per schema и DB-column length caps для schema slugs/names, а также entry `entity_type`/`status`.
- Orchestration helpers: `list/find/create/update/delete` для schemas и entries
- SeaORM adapter `FlexStandaloneSeaOrmService`
- GraphQL queries/mutations в `apps/server` для schemas и entries с отдельными `flex_schemas:*` и `flex_entries:*` permission gates
- REST endpoints в `apps/server`: `/api/v1/flex/schemas*` и `/api/v1/flex/schemas/{schema_id}/entries*` с теми же tenant-scoped RBAC gates

Для standalone surface уже действует live rollout/governance contract:

- transport остаётся server-owned adapter layer в `apps/server`, а не module-owned surface в `crates/flex`;
- `flex` зарегистрирован в `modules.toml` как `capability_only` ghost module с `rustok-module.toml` и runtime `FlexModule`;
- capability wiring проверяется через `cargo xtask validate-manifest` и `cargo xtask module validate flex`;
- multilingual DB/runtime drift проверяется через `node scripts/verify/verify-flex-multilingual-contract.mjs`;
- доступ к schemas/entries идёт только через tenant-scoped `flex_schemas:*` и `flex_entries:*` permission gates;
- server остаётся каноническим валидатором lifecycle и transport policy; thin clients не вводят свой rollout/governance contract локально.

Незакрытым остаётся полный integration verification; follow-up backlog больше не включает `indexer/cascade-delete`, но всё ещё включает расширение тестового покрытия и дальнейшую эволюцию standalone surface.

### Data model

```sql
-- Определения произвольных схем
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

- Max relation depth = 1 (нет рекурсивного populate)
- Schema slugs укладываются в `VARCHAR(64)`, schema names — в `VARCHAR(255)`, entry relation types — в `VARCHAR(64)`, entry statuses — в `VARCHAR(32)`.
- Entry statuses должны быть заранее нормализованными machine identifiers (`^[a-z][a-z0-9_]*$`) без surrounding whitespace.
- FlexEntry A может ссылаться на User/Product ✅
- FlexEntry A → FlexEntry B → FlexEntry C ❌

Подробности реализации — в [`implementation-plan.md`](./implementation-plan.md).

---

## См. также

- [`implementation-plan.md`](./implementation-plan.md) — нереализованное (Phase 4 долги, Phase 4.5, 5, 6)
- [`rustok-core/src/field_schema.rs`](../../crates/rustok-core/src/field_schema.rs) — исходный код core типов
- [`../../../docs/modules/_index.md`](../../../docs/modules/_index.md) — центральный индекс модульной документации
