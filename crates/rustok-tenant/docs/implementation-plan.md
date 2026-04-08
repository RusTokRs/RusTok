# План реализации `rustok-tenant`

Статус: базовый tenant domain contract реализован; локальная документация
приведена к единому формату и включена в scoped audit path.

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

- [ ] добавить schema validation для tenant settings;
- [ ] довести outbox events для `TenantCreated`, `TenantUpdated`, `TenantModuleToggled`;
- [ ] синхронизировать tenancy contract с RBAC для tenant-scoped admin permissions.

### 3. Operability

- [ ] довести integration tests для tenant CRUD, module toggles и resolver invariants;
- [ ] развить observability для cache hit/miss и active tenant signals;
- [ ] документировать provisioning/deprovisioning и invalidation guarantees одновременно с изменением runtime contract.

## Проверка

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- targeted tests для CRUD, module toggles, resolver invariants и cache integration path

## Правила обновления

1. При изменении tenancy contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении resolver/cache expectations обновлять также server docs.
