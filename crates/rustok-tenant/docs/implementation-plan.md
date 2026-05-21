# План реализации `rustok-tenant`

Статус: базовый tenant domain contract реализован; локальная документация
приведена к единому формату и включена в scoped audit path.

## Execution checkpoint

- Current phase: operability_observability
- Last checkpoint: Добавлены integration tests для host tenant middleware resolver path (`apps/server/tests/tenant_resolver_invariants_test.rs`) с coverage по `header/host/subdomain` resolution и `not_found/forbidden` semantics; обновлён runtime docs contract по tenant provisioning/deprovisioning и cache invalidation guarantees.
- Next step: Закрыть оставшийся contract-sync пункт в `Contract stability` и обновить verification notes по итоговому статусу модуля.
- Open blockers: None.
- Hand-off notes for next agent: Следующий инкремент сфокусировать на финальном sync tenancy invariants между `rustok-tenant` и server resolver docs, после чего перевести план в `done`.
- Last updated at (UTC): 2026-05-21T12:00:00Z

## Область работ

- удерживать `rustok-tenant` как владельца tenant domain contract;
- синхронизировать tenancy invariants, resolver expectations и local docs;
- расширять tenancy surface без смещения бизнес-логики в `apps/server`.

## Текущее состояние

- сущности `tenants` и `tenant_modules`, DTO и `TenantService` уже реализованы;
- tenant middleware resolution и cache infrastructure остаются host-owned integration path;
- module enablement уже закреплён как tenant-scoped contract;
- root `README.md`, local docs и manifest metadata входят в scoped module audit.

## Этапы

### 1. Contract stability

- [x] закрепить базовый tenant CRUD и module-toggle contract;
- [x] зафиксировать разделение ответственности между модулем и server middleware/cache layer;
- [ ] удерживать sync между tenancy invariants, server resolver path и module metadata.

### 2. Domain expansion

- [x] добавить schema validation для tenant settings (object-only JSON, depth/key/payload limits);
- [x] довести outbox events для `TenantCreated`, `TenantUpdated`, `TenantModuleToggled` (через `TransactionalEventBus` в tenant mutation flows);
- [x] синхронизировать tenancy contract с RBAC для tenant-scoped admin permissions (tenant admin bootstrap + server GraphQL tenant/module read paths выровнены по `modules:(read|list|manage)` и `tenants:(read|list|manage)` checks).

### 3. Operability

- [x] довести integration tests для tenant CRUD, module toggles и resolver invariants (baseline CRUD/module-toggle/outbox tests в `crates/rustok-tenant/tests/integration.rs`, resolver invariants в `apps/server/tests/tenant_resolver_invariants_test.rs`);
- [x] развить observability для cache hit/miss и active tenant signals (Prometheus surface дополнен `rustok_tenant_cache_coalesced_requests` + `rustok_tenant_(active|inactive|total)_total`);
- [x] документировать provisioning/deprovisioning и invalidation guarantees одновременно с изменением runtime contract.

## Проверка

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- targeted tests для CRUD, module toggles, resolver invariants и cache integration path
- контрактные тесты покрывают все публичные use-case, включая tenant CRUD, module toggles и resolver-facing invariants

## Правила обновления

1. При изменении tenancy contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении resolver/cache expectations обновлять также server docs.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
