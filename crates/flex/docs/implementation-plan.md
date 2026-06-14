# Flex — Implementation Plan

> Каноническая документация модуля: [`README.md`](./README.md)
> Центральный индекс модулей: [`docs/modules/_index.md`](/docs/modules/_index.md)

---

## Execution checkpoint

- Current phase: phase5_standalone_guardrails
- Last checkpoint: Standalone Flex guardrails tightened without compilation: schema slugs, standalone field keys and attached `entity_type` values must now be already-normalized (no trim-before-validate drift), tests cover untrimmed rejection paths, and standalone localized entry reads now filter parallel rows by tenant.
- Next step: When compilations are allowed, run `cargo test -p rustok-product --lib` first, then `cargo test -p rustok-server --lib` plus Flex-targeted integration scenarios and record evidence here.
- Open blockers: User explicitly requested no compilations for this iteration.
- Hand-off notes for next agent: Only non-compilation checks were run (`cargo fmt --all` and `node scripts/verify/verify-flex-multilingual-contract.mjs`). Verify the new standalone validation and tenant-scoped localization changes with targeted Rust tests once compilation/test execution is allowed.
- Last updated at (UTC): 2026-06-14T07:03:17Z

## Область работ

Этот план фиксирует доведение `flex` до целевого capability-only состояния в трёх плоскостях:

- attached-mode contracts и donor integrations;
- standalone schema/entry runtime и transport surfaces;
- manifest/module-system/governance contract без превращения `flex` в owner donor persistence.

## Текущее состояние

`flex` уже имеет live attached-mode contract, live standalone GraphQL/REST surfaces в `apps/server` и формализованный Phase 4.6 module-system wiring как capability-only ghost module.

## Текущий статус

> **Важная пометка для следующих change set'ов:** старые планы, где multilingual copy живёт inline в base rows или в canonical JSON blobs, больше не актуальны.
> Текущий live contract для `flex`: `FieldDefinition.is_localized` уже проведён через core/runtime/DB, registered attached consumers сейчас `user`/`product`/`order`/`topic`, standalone schema copy вынесен в `flex_schema_translations`, standalone entry values теперь разделены на `flex_entries.data` + `flex_entry_localized_values`, attached-mode generic localized-value storage вынесен в shared `crates/flex` и пишет в `flex_attached_localized_values`, а standalone GraphQL и REST surfaces для schemas/entries уже live в `apps/server`. Cleanup migration убирает residual inline locale-aware Flex payload-ы из donor metadata и standalone base rows. Rollout/governance contract для standalone уже зафиксирован через capability-only manifest, server-owned transport и repo-side verification; следующий обязательный шаг — полный integration verification и Phase 5 follow-up backlog.

| Фаза | Описание | Статус |
|------|----------|--------|
| Phase 0 | Core types & validation в `rustok-core` | ✅ Done |
| Phase 1 | Migration helper, FlexError, FieldDefRegistry, events | ✅ Done |
| Phase 2 | Users (первый потребитель) | ✅ Done |
| Phase 3 | Admin API (GraphQL CRUD, RBAC, кеш, пагинация) | ✅ Done |
| Phase 4 | Attached-mode consumers (`user`, `product`, `order`, `topic`) | ✅ Закрыто: docs/migrator/is_localized выровнены, generic localized-value storage есть, live donor read/write path закрыт для `user`, `product`, `order` и `topic` |
| Phase 4.5 | Вынос в `crates/flex` | 🔄 Почти завершён, остаются verification/docs долги |
| Phase 4.6 | Ghost-module manifest integration | ✅ Done: `modules.toml` + `rustok-module.toml` + `FlexModule` + xtask/server/docs alignment |
| Phase 5 | Standalone mode | 🔄 В активной реализации: schema-level copy, standalone entry-value split, GraphQL и REST surfaces уже live; rollout/governance contract зафиксирован, integration verification и follow-up backlog ещё не закрыты |
| Phase 6 | Advanced features | ⬜ Не начат |

---

## Этапы

Ниже фазы остаются каноническим breakdown implementation scope. Phase 4.x закрывает attached/runtime/module-system debts, Phase 5 отвечает за standalone surface, Phase 6 остаётся future backlog.

## Проверка

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`
- targeted `cargo check -p flex`

## Правила обновления

- менять статус фаз только после того, как код, docs и verification path реально синхронизированы;
- не возвращать в план старые допущения про inline localized payload как canonical path;
- staged rollout и внешние blockers фиксировать явно, а не прятать под формулировку "почти готово".

---

## Phase 4 — Долги attached mode

Flex в attached-mode уже умеет хранить field definitions и маршрутизировать CRUD по
`entity_type`, но текущее состояние неравномерное:

- `user` — полный путь schema CRUD + donor write-path validation живой.
- `product` — schema CRUD зарегистрирован в registry, donor write/read path теперь живой через shared attached localized storage.
- `order` / `topic` — schema CRUD зарегистрирован в registry, donor write/read parity уже проведён через shared attached localized storage
  нужно отдельно подтвердить или явно задокументировать как pending.
- `node` — фигурирует в модульной документации Flex как attached consumer, но в текущем registry/API
  route для `node` не смонтирован.

### Canonical scope / wiring

- [x] Зафиксировать канонический список live attached consumers
  - Live attached contract сейчас ограничен `user`, `product`, `order`, `topic`.
  - `node` не считается live attached consumer до появления отдельного service/route и donor write-path.
- [x] Выправить migrator ownership для attached migrations
  - `product_field_definitions` и `order_field_definitions` продолжают приезжать из owning crate migrations.
  - `topic_field_definitions` подключён в canonical server `Migrator`.
- [x] Зафиксировать multilingual semantics field definitions
  - `FieldDefinition.is_localized` теперь является обязательной частью core/runtime/DB контракта.
  - GraphQL, registry DTO и attached persistence больше не должны трактовать localized/non-localized поля как неявную договорённость.

### Donor write-path parity

- [x] Подтвердить и зафиксировать donor write-path integration для `order`, `topic`
  - Override 2026-04-05: `topic` is no longer schema-only. Forum topics now use `forum_topics.metadata` plus `flex_attached_localized_values` under the same attached multilingual contract as `user`/`product`/`order`.
  - Для `user` validation/defaults/strip_unknown уже подключены в GraphQL mutation flow.
  - Для `product` live read/write path уже подключён через shared attached localized storage в `crates/flex`.
  - Для оставшихся attached consumers нужно либо добавить аналогичный write-path, либо явно отметить current state как schema-only admin surface.
- [ ] Вынести localized attached values из canonical JSON path
  - `is_localized = true` не должен в финальном состоянии означать хранение multilingual business value внутри donor `metadata`.
  - Generic table `flex_attached_localized_values` уже введена, а shared entity/helpers теперь живут в `crates/flex`.
  - `user` и `product` уже используют этот path в live read/write flow.
  - Cleanup/backfill legacy inline payload-ов вынесен в отдельные миграции, runtime больше не должен читать donor/base-row inline localized copy как canonical fallback.

### Тесты (integration pending)

- [x] Flex GraphQL CRUD: интеграционные сценарии list/find/create/update/delete/reorder
  - `apps/server` теперь держит `schema.execute(...)` roundtrip для `createFieldDefinition` / `fieldDefinitions` / `fieldDefinition` / `updateFieldDefinition` / `reorderFieldDefinitions` / `deleteFieldDefinition` через live `FieldDefRegistry` routing.
- [x] Cache invalidation: integration/e2e сценарии на `FieldDefinition*` events
  - `field_definition_cache_from_context()` теперь покрыт тестом, который прогоняет invalidation через live `EventBus` subscriber на `FieldDefinitionCreated`.
- [x] RBAC integration: explicit typed permission gates для Flex surfaces
  - Standalone GraphQL/REST surfaces используют отдельные `flex_schemas:*` / `flex_entries:*` gates через `require_permission(...)` и `RequireFlex*` extractors.
  - Attached GraphQL read roots `fieldDefinitions` / `fieldDefinition` теперь тоже требуют явные `flex_schemas:list/read` права, а targeted tests фиксируют denial path.
- [x] Attached validation flows: end-to-end проверка donor write-path там, где Flex уже заявлен live
  - `rustok-order` теперь покрыт точечными create-path сценариями: shared default values, localized attached split/persist и required-field rejection.
  - `rustok-forum` теперь тоже покрыт точечными topic create/read сценариями: shared defaults, localized attached split/persist, required-field rejection и read-side resolution из `flex_attached_localized_values`.
  - `rustok-commerce` теперь покрыт точечными product create/read/update сценариями: shared defaults, localized attached split/persist, required-field rejection и locale-fallback resolution из `flex_attached_localized_values`.

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
  - Repo-side contract verification проходит: `node scripts/verify/verify-flex-multilingual-contract.mjs` = `OK`.
  - Targeted `apps/server` Flex GraphQL tests больше не завязаны на полный global migrator: локальный SQLite harness поднимает только `tenants`, `user_field_definitions`, `flex_schemas`, `flex_schema_translations`, `flex_entries` и `flex_entry_localized_values`.
  - Duplicate registration для `m20260316_000004_create_topic_field_definitions` убран из server migrator; canonical migration продолжает приезжать из `rustok_forum::migrations()`.
  - 2026-06-13 no-compile iteration: product-side metadata update path patched in `crates/rustok-product/src/services/catalog.rs` so existing reserved product metadata survives Flex custom-field PATCH-style updates; targeted helper tests were added, but not executed by request.
- [x] Синхронизировать docs с реальным registry routing и migrator ownership
  - `crates/flex/docs/README.md` выровнен по live attached consumers (`user`, `product`, `order`, `topic`) без legacy `node`.
  - GraphQL contract и RBAC section в README теперь отражают фактические `pagination`, `DeleteFieldDefinitionPayload` и typed `flex_schemas:*` / `flex_entries:*` gates.
- [x] Оставшееся server-side дублирование выделять в `crates/flex` только если это действительно transport-agnostic контракт, а не adapter concern
  - Дублировавшийся `fields_config` parser для standalone GraphQL/REST вынесен в `crates/flex::parse_field_definitions_config()`.
  - Adapter-specific pieces (`publish_event`, REST/GraphQL error mapping, response DTO mapping, RBAC extractors) осознанно оставлены в `apps/server`.

---

## Phase 4.6 — Ghost-module manifest integration

Цель: формализовать `flex` как capability / ghost module в manifest-driven module system,
а не как «обычный» доменный модуль.

### Checklist

- [x] Добавить `crates/flex/rustok-module.toml`
  - Manifest выровнен с capability-модулями наподобие `alloy`, но без претензии на donor persistence ownership.
- [x] Зафиксировать в manifest и docs семантику ghost module
  - `flex` расширяет donor modules custom contracts.
  - Данные attached-mode остаются в donor tables и donor write-path.
  - `FlexModule` публикует capability/runtime metadata и RBAC surface, а transport остаётся server-owned adapter-слоем.
- [x] Определить policy для runtime surfaces
  - Standalone GraphQL/REST уже live в `apps/server`, но в module manifest `flex` не делает вид, что это module-owned transport crate.
  - Capability-only server feature `mod-flex` нужен для registry/codegen wiring; сам crate при этом может оставаться always-linked support dependency сервера.
- [~] Прогнать manifest validation flow
  - `cargo xtask validate-manifest` / `cargo xtask module validate flex` стали частью acceptance path для `flex` и проходят на текущем workspace state
  - `cargo xtask module test flex` остаётся зависимым от общего server test graph
- [x] Обновить central module docs после появления manifest
  - `docs/modules/_index.md`
  - `docs/modules/registry.md`
  - `docs/modules/manifest.md`
  - `xtask/README.md`

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

CREATE TABLE flex_entry_localized_values (
    entry_id     UUID NOT NULL REFERENCES flex_entries(id) ON DELETE CASCADE,
    locale       VARCHAR(32) NOT NULL,
    tenant_id    UUID NOT NULL,
    data         JSONB NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entry_id, locale)
);
CREATE INDEX idx_flex_entry_localized_values_owner
    ON flex_entry_localized_values (tenant_id, entry_id);
```

### Checklist

- [~] Миграции для `flex_schemas`, `flex_entries`
  - Файл migration добавлен: `m20260317_000001_create_flex_standalone_tables`
  - Миграция подключена в canonical server migrator
  - Отдельным follow-up migration slice schema-level localized copy вынесен из `flex_schemas` в `flex_schema_translations`
  - Отдельным follow-up migration slice standalone localized entry payload вынесен из inline `flex_entries.data` в `flex_entry_localized_values`
- [x] SeaORM entities *(добавлены `flex_schemas`, `flex_entries`, `flex_schema_translations` и `flex_entry_localized_values` в `apps/server/src/models/_entities` + re-export в `models/`)*
- [x] Validation service (использует `CustomFieldsSchema` из core) *(добавлен `apps/server/src/services/flex_standalone_validation_service.rs`, включая normalize/apply_defaults/strip_unknown/validate pipeline)*
- [x] CRUD services *(добавлен SeaORM adapter `FlexStandaloneSeaOrmService` в `apps/server/src/services/flex_standalone_service.rs`, реализующий `flex::FlexStandaloneService` с tenant-scoped CRUD для schemas/entries)*
- [x] Multilingual storage contract для standalone mode
  - schema-level localized copy (`name`, `description`) больше не считается base-row данными
  - `flex_schema_translations` уже является live storage path для schema-level copy
  - entry payload теперь split на `flex_entries.data` (shared) и `flex_entry_localized_values` (locale-aware values)
  - read/write service path уже мерджит parallel localized rows обратно в effective entry payload
  - cleanup/backfill вынесен в follow-up migrations; runtime читает shared payload плюс parallel localized rows
- [x] Events: `FlexSchemaCreated/Updated/Deleted`, `FlexEntryCreated/Updated/Deleted` *(event contracts + schema registry добавлены в `rustok-events`; `crates/flex` даёт transport-agnostic envelope helper-ы и orchestration helper-ы `*_with_event()`, а GraphQL/REST adapters в `apps/server` уже публикуют эти envelopes в event bus)*
- [x] REST API: `/api/v1/flex/schemas`, `/api/v1/flex/schemas/{schema_id}/entries` *(live в `apps/server`, tenant-scoped и с отдельными `flex_schemas:*` / `flex_entries:*` permission gates)*
- [x] GraphQL: `FlexSchema`, `FlexEntry`, queries/mutations *(live в `apps/server`, tenant-scoped и с отдельными `flex_schemas:*` / `flex_entries:*` permission gates)*
- [x] RBAC permissions: `flex.schemas.*`, `flex.entries.*`
  - Typed permissions есть в `rustok-core`
  - GraphQL standalone surface использует отдельные `flex_schemas:*` и `flex_entries:*` gates
- [x] Indexer handler: `index_flex_entries` + `FlexIndexer` event handler
  - `rustok-index` теперь владеет migration slice `index_flex_entries` и module-owned `flex_indexer`, который слушает `FlexEntry*`, `FlexSchemaUpdated/Deleted` и `ReindexRequested { target_type = "flex" }`.
  - `IndexModule` публикует `flex_indexer` через `register_event_listeners(...)`, а server dispatcher включает его в runtime wiring наравне с `content_indexer` и `product_indexer`.
- [x] Cascade delete: при удалении entity удалять attached flex entries
  - Shared helper `delete_attached_localized_values()` живёт в `crates/flex` и подключён в live hard-delete paths для `user`, `product` и `topic`.
  - Helper допускает capability-optional test graphs без смонтированной таблицы `flex_attached_localized_values`, чтобы isolated donor tests не падали на cleanup-пути.
  - Для `order` отдельный hard-delete surface в текущем live contract не реализован; cleanup будет нужен сразу при появлении такого delete-path.
- [x] Guardrail: max relation depth = 1 (no recursive populate)
  - `crates/flex::validate_create_entry_command()` теперь явно запрещает `entity_type = "flex_entry"`, так что standalone `FlexEntry -> FlexEntry` цепочки режутся до adapter/service layer и одинаково работают для GraphQL и REST.
- [x] Решить publish policy для standalone surface через ghost-module manifest
  - Standalone surface остаётся server-owned adapter layer.
  - `flex` публикует capability/runtime metadata через `rustok-module.toml`, `modules.toml` и `FlexModule`, не забирая ownership transport surface.
  - Acceptance path: `cargo xtask validate-manifest`, `cargo xtask module validate flex`, `node scripts/verify/verify-flex-multilingual-contract.mjs`.
- [ ] Тесты: unit + integration
  - `apps/server` уже держит targeted REST roundtrip для standalone schema/entry CRUD и invalid payload rejection.
  - `apps/server` теперь также держит standalone GraphQL roundtrip для schema/entry CRUD и explicit denial-path для `flex_entries:create`.
  - Flex GraphQL tests в `apps/server` теперь используют isolated SQLite bootstrap вместо полного workspace migrator, чтобы не тянуть посторонние migration slices в flex verification path.
  - Repo-side multilingual drift gate проходит: `node scripts/verify/verify-flex-multilingual-contract.mjs`.
  - 2026-06-14 no-compile iteration: standalone contract guardrail tests added for untrimmed schema slugs, field keys and `entity_type`; localized entry row loading now includes tenant filtering to keep the parallel storage lookup tenant-scoped.
  - Полное закрытие пункта всё ещё требует стабильный `rustok-server` test run; текущий инкремент подготовил standalone guardrail fix и тесты, но compile/test evidence отложен, потому что эта итерация выполнялась без компиляций.
- [x] Документация
  - Контракты, data model и live GraphQL/REST surfaces описаны
  - Rollout / governance contract для standalone surface задокументирован как completed

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
> **Live status override (2026-04-05):** attached multilingual donor path уже реально закрыт для `user`, `product`, `order` и `topic` через shared `flex_attached_localized_values`.
> `topic` больше не является schema-level consumer: forum topic donor payload теперь живёт в `forum_topics.metadata`, а locale-aware Flex values резолвятся по тому же effective locale contract, что и у остальных live donors.
> Если нижележащие разделы старого плана говорят, что `order` ещё не переведён или что для `topic` уже существует donor metadata path, считать это устаревшим.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
