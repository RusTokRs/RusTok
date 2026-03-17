# Flex — Implementation Plan

> Документация модуля: [`docs/modules/flex.md`](/docs/modules/flex.md)
> Архитектура: [`docs/architecture/flex.md`](/docs/architecture/flex.md)

---

## Текущий статус

| Фаза | Описание | Статус |
|------|----------|--------|
| Phase 0 | Core types & validation в `rustok-core` | ✅ Done |
| Phase 1 | Migration helper, FlexError, FieldDefRegistry, events | ✅ Done |
| Phase 2 | Users (первый потребитель) | ✅ Done |
| Phase 3 | Admin API (GraphQL CRUD, RBAC, кеш, пагинация) | ✅ Done |
| Phase 4 | Commerce, Content, Forum | 🔄 В основном выполнено, есть долги |
| Phase 4.5 | Вынос в `crates/flex` | 🔄 В процессе |
| Phase 5 | Standalone mode | ⬜ Не начат |
| Phase 6 | Advanced features | ⬜ Не начат |

---

## Phase 4 — Долги (Commerce, Content, Forum)

### Pre-req

- [ ] Добавить `metadata JSONB` колонку в таблицу `orders`
  - Нужна миграция в `apps/server/migration/`
  - Без этого `order_field_definitions` таблица создана, но данные некуда писать

### Тесты (integration pending)

- [ ] `UserFieldService`: интеграционные CRUD/validation сценарии
  - unit-тесты: guardrails, events, not-found ветки — есть
  - integration: GraphQL CRUD + validation flow — pending
- [ ] GraphQL Admin API: интеграционные сценарии RBAC, cache invalidation, pagination
  - unit-тесты: есть
  - integration: pending

---

## Phase 4.5 — Вынос в `crates/flex`

Цель: переместить generic attached-mode контракты из `apps/server` в `crates/flex`,
оставив в `apps/server` только transport/adapters (GraphQL, RBAC gate, bootstrap wiring).

**Go/No-Go критерии для старта:**
1. Закрыт pre-req по `orders.metadata`
2. Есть полный интеграционный прогон Flex GraphQL CRUD + cache invalidation
3. Нет незакрытых P1-багов по текущей registry маршрутизации

### Что уже перенесено

- [x] `crates/flex` создан
- [x] Registry contracts (`FieldDefinitionService`, `FieldDefRegistry` adapter layer)
- [x] Generic CRUD orchestration helpers (registry lookup + CRUD/reorder routing)
- [x] `apps/server` использует прямые импорты из `flex` (без compatibility re-export)

### Что осталось перенести

- [x] Cache invalidation hooks
  - В `crates/flex` добавлены orchestration helpers `list_field_definitions_with_cache()` и `invalidate_field_definition_cache()`
  - `apps/server` реализует `FieldDefinitionCachePort` как adapter над текущим cache implementation
- [x] Transport-agnostic error mapping
  - `map_flex_error()` перенесён в `crates/flex/src/errors.rs`
  - `apps/server` использует mapping из agnostic-модуля
- [x] Перевести `user/product/order/topic` сервисы на новый crate API
  - Bootstrap/GraphQL используют прямой API `flex` без изменения GraphQL-контрактов
- [ ] Убрать дублирование между `apps/server` и `crates/flex`
- [x] Написать migration guide: `apps/server/docs/` + cross-link в `docs/index.md`

---

## Phase 5 — Standalone mode

Произвольные схемы и записи без привязки к существующим сущностям.

### Таблицы

```sql
CREATE TABLE flex_schemas (
    id            UUID PRIMARY KEY,
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug          VARCHAR(64) NOT NULL,
    name          VARCHAR(255) NOT NULL,
    description   TEXT,
    fields_config JSONB NOT NULL,
    settings      JSONB NOT NULL DEFAULT '{}',
    is_active     BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, slug)
);

CREATE TABLE flex_entries (
    id          UUID PRIMARY KEY,
    tenant_id   UUID NOT NULL,
    schema_id   UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    entity_type VARCHAR(64),    -- NULL = standalone
    entity_id   UUID,           -- NULL = standalone
    data        JSONB NOT NULL,
    status      VARCHAR(32) NOT NULL DEFAULT 'draft',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, schema_id, entity_type, entity_id)
        WHERE entity_type IS NOT NULL AND entity_id IS NOT NULL
);
CREATE INDEX idx_flex_entries_data   ON flex_entries USING GIN (data);
CREATE INDEX idx_flex_entries_entity ON flex_entries (entity_type, entity_id);
```

### Checklist

- [ ] Миграции для `flex_schemas`, `flex_entries`
- [ ] SeaORM entities
- [ ] Validation service (использует `CustomFieldsSchema` из core)
- [ ] CRUD services
- [ ] Events: `FlexSchemaCreated/Updated/Deleted`, `FlexEntryCreated/Updated/Deleted`
- [ ] REST API: `/api/v1/flex/schemas`, `/api/v1/flex/schemas/:slug/entries`
- [ ] GraphQL: `FlexSchema`, `FlexEntry`, queries/mutations
- [ ] RBAC permissions: `flex.schemas.*`, `flex.entries.*` → добавить в `RusToKModule::permissions()`
- [ ] Indexer handler: `index_flex_entries` + `FlexIndexer` event handler
- [ ] Cascade delete: при удалении entity удалять attached flex entries
- [ ] Guardrail: max relation depth = 1 (no recursive populate)
- [ ] Тесты: unit + integration
- [ ] Документация

### События standalone mode

```rust
DomainEvent::FlexSchemaCreated { schema_id, slug }
DomainEvent::FlexSchemaUpdated { schema_id, slug }
DomainEvent::FlexSchemaDeleted { schema_id }
DomainEvent::FlexEntryCreated { schema_id, entry_id, entity_type, entity_id }
DomainEvent::FlexEntryUpdated { schema_id, entry_id }
DomainEvent::FlexEntryDeleted { schema_id, entry_id }
```

### Read model (indexer)

```sql
CREATE TABLE index_flex_entries (
    id            UUID PRIMARY KEY,
    tenant_id     UUID NOT NULL,
    schema_slug   VARCHAR(64) NOT NULL,
    entity_type   VARCHAR(64),
    entity_id     UUID,
    data_preview  JSONB,
    search_vector TSVECTOR,
    indexed_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, id)
);
CREATE INDEX idx_index_flex_search ON index_flex_entries USING GIN (search_vector);
```

### Open questions

1. **Schema versioning:** нужна ли история изменений схем?
2. **Migration on schema change:** как мигрировать данные при изменении полей?
3. **Rich text fields:** поддерживать ли Markdown/HTML в text полях?
4. **Computed fields:** нужны ли поля, вычисляемые на лету?

---

## Phase 6 — Advanced (future)

- [ ] Conditional fields (показывать поле B если поле A = X)
- [ ] Computed fields (вычисляемые из других полей)
- [ ] Field groups (секции в UI)
- [ ] Import/export схем между тенантами
- [ ] Full-text search по custom fields через rustok-index
- [ ] Schema versioning (история изменений определений)
- [ ] Data migration tool (ретро-валидация существующих metadata)

---

## Tracking

При изменении плана:
1. Обновить этот файл
2. Обновить статусы в [`docs/modules/flex.md`](/docs/modules/flex.md)
3. Запустить `cargo test -p rustok-core` — тесты field_schema должны проходить
