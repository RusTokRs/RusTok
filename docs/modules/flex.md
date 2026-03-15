# Flex Module Specification

> **Новый концепт из архитектурного обсуждения 2026-02-06**  
> **Статус:** Concept / Draft

---

## 1. Обзор

### 1.1 Что такое Flex?

**Flex (Generic Content Builder / Data Builder)** — опциональный вспомогательный модуль-конструктор данных, созданный по итогам архитектурного обсуждения.

Flex нужен для редких кастомных кейсов, когда:

- Стандартных доменных модулей (content, commerce, blog) **недостаточно**
- Создавать отдельный доменный модуль **нецелесообразно**
- Бизнес хочет "кастомные поля/структуры" без программирования

### 1.2 Происхождение

Идея Flex появилась в ходе обсуждения архитектуры RusToK с AI-ассистентами. Основной вопрос был: "Как добавить гибкость системе (как Strapi/Directus), но не превратить её в помойку JSONB и не потерять производительность?"

**Решение:** Flex существует **рядом** со стандартными модулями, а не **вместо** них. Это "запасной выход" для edge-cases, а не основа платформы.

---

## 2. Purpose & Non-Goals

### 2.1 Purpose (для чего)

✅ Runtime-определяемые схемы данных  
✅ Хранение записей в JSONB  
✅ Attached behavior — прикрепление данных к сущностям других модулей  
✅ Интеграция через события и индексатор  
✅ Маркетинговые лендинги, формы, баннеры  
✅ Справочники с произвольными полями  

### 2.2 Non-Goals (для чего НЕ)

❌ Замена стандартных модулей (content, commerce, blog)  
❌ Хранение критических данных (заказы, платежи, инвентарь)  
❌ Создание сложных реляционных связей  
❌ Альтернатива нормализованным таблицам  

---

## 3. Architectural Rules (HARD LAWS)

> Эти правила **не подлежат обсуждению**.

| # | Правило | Обоснование |
|---|---------|-------------|
| **1** | **Standard modules NEVER depend on Flex** | Flex — опция, не зависимость |
| **2** | Flex depends only on `rustok-core` | Минимальный surface area |
| **3** | **Removal-safe** | Удаление Flex не ломает платформу |
| **4** | Read через indexer, не runtime JOIN | Performance storefront |
| **5** | No Flex in critical domains | Orders/payments/inventory — запрещено |

### 3.1 Dependency Diagram

```text
rustok-core
    ↑
rustok-flex (optional)
    
rustok-commerce ←✗→ rustok-flex  (NO dependency!)
rustok-content  ←✗→ rustok-flex  (NO dependency!)
```

---

## 4. Use Cases

### 4.1 Когда использовать Flex

| Сценарий | Пример |
|----------|--------|
| Кастомные поля к товарам | Дополнительные характеристики для специфической ниши |
| Маркетинговые лендинги | Страницы с кастомной структурой |
| Простые формы | Feedback, опросы без кода |
| Внутренние справочники | База знаний с уникальной таксономией |
| A/B тестирование | Кастомные баннеры/блоки |

### 4.2 Когда НЕ использовать Flex

| Сценарий | Что использовать |
|----------|------------------|
| Блог | `rustok-blog` |
| Страницы | `rustok-pages` |
| Товары | `rustok-commerce` |
| SEO-данные | Стандартные поля в модулях |
| Заказы/оплаты | `rustok-commerce` |

**Правило:** Если кейс закрывается стандартным модулем — используй стандартный модуль.

---

## 5. Data Model

### 5.1 Schemas (определения типов)

```sql
CREATE TABLE flex_schemas (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug            VARCHAR(64) NOT NULL,      -- 'landing-page', 'feedback-form'
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    fields_config   JSONB NOT NULL,            -- Описание полей
    settings        JSONB NOT NULL DEFAULT '{}',
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE (tenant_id, slug)
);
```

### 5.2 Entries (записи данных)

```sql
CREATE TABLE flex_entries (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    schema_id       UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    
    -- Attached behavior (полиморфная связь)
    entity_type     VARCHAR(64),               -- 'product', 'user', 'order', NULL
    entity_id       UUID,                      -- ID сущности, NULL если standalone
    
    data            JSONB NOT NULL,            -- Сами данные
    status          VARCHAR(32) NOT NULL DEFAULT 'draft',
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE (tenant_id, schema_id, entity_type, entity_id) 
        WHERE entity_type IS NOT NULL AND entity_id IS NOT NULL
);

CREATE INDEX idx_flex_entries_data ON flex_entries USING GIN (data);
CREATE INDEX idx_flex_entries_entity ON flex_entries (entity_type, entity_id);
CREATE INDEX idx_flex_entries_tenant_schema ON flex_entries (tenant_id, schema_id);
```

### 5.3 Fields Config Structure

```json
{
  "fields": [
    {
      "name": "hero_title",
      "type": "string",
      "required": true,
      "max_length": 255
    },
    {
      "name": "hero_image",
      "type": "media",
      "required": false
    },
    {
      "name": "price",
      "type": "number",
      "required": false,
      "min": 0
    },
    {
      "name": "features",
      "type": "array",
      "items": { "type": "string" },
      "max_items": 20
    }
  ]
}
```

---

## 6. Guardrails (обязательные ограничения)

| Guardrail | Значение | Статус |
|-----------|----------|--------|
| Max fields per schema | 50 | ⬜ TODO |
| Max nesting depth | 2 | ⬜ TODO |
| Max relation depth | 1 (no recursive populate) | ⬜ TODO |
| Mandatory pagination | Да | ⬜ TODO |
| Validation on write | Строгая по schema | ⬜ TODO |
| Timeout for operations | 5s | ⬜ TODO |

### 6.1 Validation Contract

```rust
pub fn validate_entry(schema: &FlexSchema, data: &serde_json::Value) -> Result<(), ValidationError> {
    // 1. Проверить required fields
    // 2. Проверить типы
    // 3. Проверить constraints (max_length, min, max_items)
    // 4. НЕ допускать лишних полей
}
```

- ⬜ TODO: Реализовать валидацию
- ⬜ TODO: Тесты на все edge-cases

---

## 7. Events

### 7.1 Emitted Events

```rust
// В rustok-core/src/events/types.rs

// FLEX EVENTS
FlexSchemaCreated { schema_id: Uuid, slug: String },
FlexSchemaUpdated { schema_id: Uuid, slug: String },
FlexSchemaDeleted { schema_id: Uuid },

FlexEntryCreated { 
    schema_id: Uuid, 
    entry_id: Uuid, 
    entity_type: Option<String>,
    entity_id: Option<Uuid>,
},
FlexEntryUpdated { schema_id: Uuid, entry_id: Uuid },
FlexEntryDeleted { schema_id: Uuid, entry_id: Uuid },
```

### 7.2 Consumed Events

Flex может слушать события других модулей для автоматической привязки:

```rust
// Когда удаляется product, удаляем attached flex entries
ProductDeleted { product_id } => {
    flex_service.delete_attached(entity_type: "product", entity_id: product_id)
}
```

- ⬜ TODO: Определить политику cascade delete

---

## 8. APIs

### 8.1 REST Endpoints

```
# Schemas
GET    /api/v1/flex/schemas
POST   /api/v1/flex/schemas
GET    /api/v1/flex/schemas/:slug
PUT    /api/v1/flex/schemas/:slug
DELETE /api/v1/flex/schemas/:slug

# Entries
GET    /api/v1/flex/schemas/:slug/entries
POST   /api/v1/flex/schemas/:slug/entries
GET    /api/v1/flex/schemas/:slug/entries/:id
PUT    /api/v1/flex/schemas/:slug/entries/:id
DELETE /api/v1/flex/schemas/:slug/entries/:id

# Attached entries
GET    /api/v1/flex/attached/:entity_type/:entity_id
```

### 8.2 GraphQL

```graphql
type FlexSchema {
  id: ID!
  slug: String!
  name: String!
  fieldsConfig: JSON!
}

type FlexEntry {
  id: ID!
  schema: FlexSchema!
  data: JSON!
  entityType: String
  entityId: ID
}

type Query {
  flexSchemas: [FlexSchema!]!
  flexSchema(slug: String!): FlexSchema
  flexEntries(schemaSlug: String!, pagination: Pagination): FlexEntryConnection!
  flexEntry(schemaSlug: String!, id: ID!): FlexEntry
}

type Mutation {
  createFlexSchema(input: CreateFlexSchemaInput!): FlexSchema!
  updateFlexSchema(slug: String!, input: UpdateFlexSchemaInput!): FlexSchema!
  deleteFlexSchema(slug: String!): Boolean!
  
  createFlexEntry(schemaSlug: String!, input: CreateFlexEntryInput!): FlexEntry!
  updateFlexEntry(schemaSlug: String!, id: ID!, input: UpdateFlexEntryInput!): FlexEntry!
  deleteFlexEntry(schemaSlug: String!, id: ID!): Boolean!
}
```

---

## 9. RBAC Permissions

```rust
Permission::new("flex.schemas.read"),
Permission::new("flex.schemas.write"),
Permission::new("flex.schemas.delete"),
Permission::new("flex.entries.read"),
Permission::new("flex.entries.write"),
Permission::new("flex.entries.delete"),
```

- ⬜ TODO: Добавить в RusToKModule::permissions()

---

## 10. Indexing Strategy

### 10.1 Read Model Table

```sql
CREATE TABLE index_flex_entries (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    schema_slug     VARCHAR(64) NOT NULL,
    schema_name     VARCHAR(255) NOT NULL,
    entity_type     VARCHAR(64),
    entity_id       UUID,
    data_preview    JSONB,                     -- Сжатое представление
    search_vector   TSVECTOR,                  -- Для поиска
    indexed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE (tenant_id, id)
);

CREATE INDEX idx_index_flex_search ON index_flex_entries USING GIN (search_vector);
```

### 10.2 Indexer Handler

```rust
impl EventHandler for FlexIndexer {
    async fn handle(&self, envelope: &EventEnvelope) -> Result<()> {
        match &envelope.event {
            DomainEvent::FlexEntryCreated { entry_id, .. } |
            DomainEvent::FlexEntryUpdated { entry_id, .. } => {
                self.index_entry(*entry_id).await
            }
            DomainEvent::FlexEntryDeleted { entry_id, .. } => {
                self.remove_from_index(*entry_id).await
            }
            _ => Ok(())
        }
    }
}
```

---

## 11. Implementation Checklist

| # | Задача | Статус |
|---|--------|--------|
| 11.1 | Создать crate `rustok-flex` | ⬜ TODO |
| 11.2 | Миграции для `flex_schemas`, `flex_entries` | ⬜ TODO |
| 11.3 | SeaORM entities | ⬜ TODO |
| 11.4 | Validation service | ⬜ TODO |
| 11.5 | CRUD services | ⬜ TODO |
| 11.6 | Events integration | ⬜ TODO |
| 11.7 | REST controllers | ⬜ TODO |
| 11.8 | GraphQL resolvers | ⬜ TODO |
| 11.9 | Indexer handler | ⬜ TODO |
| 11.10 | Admin UI integration | ⬜ TODO |
| 11.11 | Tests: unit + integration | ⬜ TODO |
| 11.12 | Documentation | ⬜ TODO |

---

## 12. Open Questions

1. **Versioning schemas:** Нужна ли история изменений схем?
2. **Migration on schema change:** Как мигрировать данные при изменении полей?
3. **Rich text fields:** Поддерживать ли Markdown/HTML в text полях?
4. **Computed fields:** Нужны ли поля, вычисляемые на лету?

---

## См. также

- [ARCHITECTURE_GUIDE.md](../ARCHITECTURE_GUIDE.md) — общая архитектура
- [ROADMAP.md](../ROADMAP.md) — фазы разработки

