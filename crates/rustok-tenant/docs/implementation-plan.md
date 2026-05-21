# План реализации `rustok-tenant`

Статус: базовый tenant domain contract стабилизирован; текущая итерация
переведена в планирование следующего tenant-domain инкремента.

## Execution checkpoint

- Current phase: iteration_2_planning
- Last checkpoint: Закрыт оставшийся contract-sync по tenancy invariants между `rustok-tenant` (`README.md`, `docs/README.md`, `rustok-module.toml`) и host resolver contract в `apps/server/docs/README.md`; verification gates обновлены под фактическое покрытие (`rustok-tenant` + server resolver invariants).
- Next step: Стартовать Iteration 2 с hardening provisioning/deprovisioning path — добавить integration coverage для обязательного cache invalidation после create/update/deactivate/domain-change.
- Open blockers: None.
- Hand-off notes for next agent: Не расширять scope на новый tenant feature set; в этой итерации держать фокус на lifecycle consistency и regression safety между модулем и host middleware/cache path.
- Last updated at (UTC): 2026-05-21T13:30:00Z

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
- [x] удерживать sync между tenancy invariants, server resolver path и module metadata.

### 2. Domain expansion

- [x] добавить schema validation для tenant settings (object-only JSON, depth/key/payload limits);
- [x] довести outbox events для `TenantCreated`, `TenantUpdated`, `TenantModuleToggled` (через `TransactionalEventBus` в tenant mutation flows);
- [x] синхронизировать tenancy contract с RBAC для tenant-scoped admin permissions (tenant admin bootstrap + server GraphQL tenant/module read paths выровнены по `modules:(read|list|manage)` и `tenants:(read|list|manage)` checks).

### 3. Operability

- [x] довести integration tests для tenant CRUD, module toggles и resolver invariants (baseline CRUD/module-toggle/outbox tests в `crates/rustok-tenant/tests/integration.rs`, resolver invariants в `apps/server/tests/tenant_resolver_invariants_test.rs`);
- [x] развить observability для cache hit/miss и active tenant signals (Prometheus surface дополнен `rustok_tenant_cache_coalesced_requests` + `rustok_tenant_(active|inactive|total)_total`);
- [x] документировать provisioning/deprovisioning и invalidation guarantees одновременно с изменением runtime contract.

### 4. Iteration 2 — tenant lifecycle hardening

- [ ] добавить integration coverage для host provisioning/deprovisioning path: после create/update/deactivate/domain-change обязательно проверять invalidation хуков `invalidate_tenant_cache_by_uuid/slug/host`;
- [ ] расширить server resolver regression matrix под lifecycle invalidation (positive + negative cache сценарии после tenant state transition);
- [ ] зафиксировать migration note по deprecated `TenantService::toggle_module`: runtime module enable/disable path должен идти через host `ModuleLifecycleService`.

## Проверка

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant --tests`
- `cargo test -p rustok-server --test tenant_resolver_invariants_test`
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
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.
- [ ] Добавить lifecycle-focused integration checks для cache invalidation после tenant state transitions.
