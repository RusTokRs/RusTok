# Flex — Implementation Plan

> Каноническая документация модуля: [`README.md`](./README.md)
> Центральный индекс модулей: [`docs/modules/_index.md`](/docs/modules/_index.md)

---

## Текущий статус

| Фаза | Описание | Статус |
|------|----------|--------|
| Phase 0 | Core types & validation в `rustok-core` | ✅ Done |
| Phase 1 | Migration helper, FlexError, FieldDefRegistry, events | ✅ Done |
| Phase 2 | Users (первый потребитель) | ✅ Done |
| Phase 3 | Admin API (GraphQL CRUD, RBAC, кеш, пагинация) | ✅ Done |
| Phase 4 | Attached-mode consumers (`user`, `product`, `order`, `topic`) | 🔄 Частично готово, есть расхождения между docs, migrator wiring и donor write-path |
| Phase 4.5 | Вынос в `crates/flex` | 🔄 Почти завершён, остаются verification/docs долги |
| Phase 4.6 | Ghost-module manifest integration | ⬜ Не начат |
| Phase 5 | Standalone mode | 🔄 Начат (контракты и adapter-layer есть; multilingual storage split для schema-level copy начат, live API surface ещё не опубликован) |
| Phase 6 | Advanced features | ⬜ Не начат |

---

## Phase 4 — Долги attached mode

Flex в attached-mode уже умеет хранить field definitions и маршрутизировать CRUD по
`entity_type`, но текущее состояние неравномерное:

- `user` — полный путь schema CRUD + donor write-path validation живой.
- `product` / `order` / `topic` — schema CRUD зарегистрирован в registry, но donor write-path parity
  нужно отдельно подтвердить или явно задокументировать как pending.
- `node` — фигурирует в модульной документации Flex как attached consumer, но в текущем registry/API
  route для `node` не смонтирован.

### Canonical scope / wiring

- [ ] Зафиксировать канонический список live attached consumers
  - В docs сейчас заявлены `user`, `product`, `order`, `node`, `topic`.
  - В registry/API реально смонтированы `user`, `product`, `order`, `topic`.
  - Нужно либо добавить `node` service/route, либо убрать `node` из live-contract документации.
- [ ] Выправить migrator ownership для attached migrations
  - `product_field_definitions` и `order_field_definitions` приезжают из owning crate migrations.
  - `topic_field_definitions` лежит в `apps/server/migration`, но не подключён в `Migrator`.
  - Нужно либо зарегистрировать server migration, либо перенести ownership migration в owning crate.

### Donor write-path parity

- [ ] Подтвердить и зафиксировать donor write-path integration для `product`, `order`, `topic`
  - Для `user` validation/defaults/strip_unknown уже подключены в GraphQL mutation flow.
  - Для остальных attached consumers нужно либо добавить аналогичный write-path, либо явно отметить current state как schema-only admin surface.

### Тесты (integration pending)

- [ ] Flex GraphQL CRUD: интеграционные сценарии list/find/create/update/delete/reorder
- [ ] Cache invalidation: integration/e2e сценарии на `FieldDefinition*` events
- [ ] RBAC integration: explicit typed permission gates для Flex surfaces
- [ ] Attached validation flows: end-to-end проверка donor write-path там, где Flex уже заявлен live

---

## Phase 4.5 — Вынос в `crates/flex`

Цель: переместить generic attached-mode контракты из `apps/server` в `crates/flex`,
оставив в `apps/server` только transport/adapters (GraphQL, RBAC gate, bootstrap wiring).

**Go/No-Go критерии для старта:**
1. Закрыты attached-mode wiring долги по live consumers
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
- [x] Удалён legacy-дубликат `crates/rustok-flex`
  - В workspace остаётся единый agnostic модуль `crates/flex`
- [x] Убрать дублирование между `apps/server` и `crates/flex`
- [x] Написать migration guide: `apps/server/docs/` + cross-link в `docs/index.md`

### Что осталось закрыть перед финализацией phase

- [ ] Полный integration прогон GraphQL CRUD + cache invalidation
- [ ] Синхронизировать docs с реальным registry routing и migrator ownership
- [ ] Оставшееся server-side дублирование выделять в `crates/flex` только если это действительно transport-agnostic контракт, а не adapter concern

---

## Phase 4.6 — Ghost-module manifest integration

Цель: формализовать `flex` как capability / ghost module в manifest-driven module system,
а не как «обычный» доменный модуль.

### Checklist

- [ ] Добавить `crates/flex/rustok-module.toml`
  - Минимальный contract должен быть выровнен с capability-модулями наподобие `alloy`.
  - Manifest не должен заявлять ownership над donor persistence (`users.metadata`, `orders.metadata`, `nodes.metadata` и т.д.).
- [ ] Зафиксировать в manifest и docs семантику ghost module
  - `flex` расширяет donor modules custom contracts.
  - Данные attached-mode остаются в donor tables и donor write-path.
  - Сам `flex` поставляет shared orchestration / runtime capability, а не новый bounded context.
- [ ] Определить policy для runtime surfaces
  - Если standalone API ещё не live, manifest не должен притворяться publish-ready модулем с завершённым GraphQL/HTTP contract.
  - Если standalone surface будет открываться поэтапно, manifest и docs должны явно отражать staged rollout.
- [ ] Прогнать manifest validation flow
  - `cargo xtask module validate flex`
  - `cargo xtask module test flex`
- [ ] Обновить central module docs после появления manifest
  - `docs/modules/_index.md`
  - `docs/modules/registry.md`
  - при необходимости `docs/modules/manifest.md`

---

## Phase 5 — Standalone mode

Произвольные схемы и записи без привязки к существующим сущностям.

### Что уже начато

- [x] Добавлены transport-agnostic standalone контракты в `crates/flex/src/standalone.rs`
  - DTO для схем/записей (`FlexSchemaView`, `FlexEntryView`)
  - Commands и trait-контракт `FlexStandaloneService` для будущих adapter-реализаций
  - Базовые guardrail validators для create/update-команд (`validate_create_schema_command`, `validate_update_schema_command`, `validate_create_entry_command`, `validate_update_entry_command`)
  - Orchestration helpers (`list/find/create/update/delete` для schemas и entries), чтобы adapters не дублировали routing/pre-validation

### Таблицы

```sql
CREATE TABLE flex_schemas (
    id            UUID PRIMARY KEY,
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug          VARCHAR(64) NOT NULL,
    fields_config JSONB NOT NULL,
    settings      JSONB NOT NULL DEFAULT '{}',
    is_active     BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, slug)
);

CREATE TABLE flex_schema_translations (
    schema_id     UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    locale        VARCHAR(32) NOT NULL,
    name          VARCHAR(255) NOT NULL,
    description   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (schema_id, locale)
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

- [~] Миграции для `flex_schemas`, `flex_entries`
  - Файл migration добавлен: `m20260317_000001_create_flex_standalone_tables`
  - Миграция подключена в canonical server migrator
  - Отдельным follow-up migration slice schema-level localized copy вынесен из `flex_schemas` в `flex_schema_translations`
- [x] SeaORM entities *(добавлены `flex_schemas` и `flex_entries` в `apps/server/src/models/_entities` + re-export в `models/`)*
- [x] Validation service (использует `CustomFieldsSchema` из core) *(добавлен `apps/server/src/services/flex_standalone_validation_service.rs`, включая normalize/apply_defaults/strip_unknown/validate pipeline)*
- [x] CRUD services *(добавлен SeaORM adapter `FlexStandaloneSeaOrmService` в `apps/server/src/services/flex_standalone_service.rs`, реализующий `flex::FlexStandaloneService` с tenant-scoped CRUD для schemas/entries)*
- [~] Multilingual storage contract для standalone mode
  - schema-level localized copy (`name`, `description`) больше не считается base-row данными
  - entry payload `data` пока остаётся JSONB без split на localized/non-localized values
  - следующий обязательный шаг: явная semantics `localized/non-localized` для field definitions и parallel localized records для entry values
- [~] Events: `FlexSchemaCreated/Updated/Deleted`, `FlexEntryCreated/Updated/Deleted` *(event contracts + schema registry добавлены в `rustok-events`; в `crates/flex` добавлены transport-agnostic envelope helper-ы и orchestration helper-ы `*_with_event()`, emission wiring в adapters pending)*
- [ ] REST API: `/api/v1/flex/schemas`, `/api/v1/flex/schemas/{slug}/entries`
- [ ] GraphQL: `FlexSchema`, `FlexEntry`, queries/mutations
- [~] RBAC permissions: `flex.schemas.*`, `flex.entries.*`
  - Typed permissions уже есть в `rustok-core`
  - Standalone surfaces пока не используют полный `flex.entries.*` contract
- [ ] Indexer handler: `index_flex_entries` + `FlexIndexer` event handler
- [ ] Cascade delete: при удалении entity удалять attached flex entries
- [ ] Guardrail: max relation depth = 1 (no recursive populate)
- [ ] Решить publish policy для standalone surface через ghost-module manifest
- [ ] Тесты: unit + integration
- [~] Документация
  - Контракты и data model описаны
  - Live API / rollout / governance contract для standalone surface ещё не задокументирован как completed

### События standalone mode

```rust
DomainEvent::FlexSchemaCreated { tenant_id, schema_id, slug }
DomainEvent::FlexSchemaUpdated { tenant_id, schema_id, slug }
DomainEvent::FlexSchemaDeleted { tenant_id, schema_id }
DomainEvent::FlexEntryCreated { tenant_id, schema_id, entry_id, entity_type, entity_id }
DomainEvent::FlexEntryUpdated { tenant_id, schema_id, entry_id }
DomainEvent::FlexEntryDeleted { tenant_id, schema_id, entry_id }
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
2. Обновить ссылки и статус в [`docs/modules/_index.md`](/docs/modules/_index.md) или [`docs/modules/registry.md`](/docs/modules/registry.md), если меняется состав/статус модуля
3. Запустить `cargo test -p rustok-core` — тесты field_schema должны проходить
