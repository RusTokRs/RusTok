# Документация `rustok-tenant`

`rustok-tenant` — канонический tenancy-модуль платформы. Он задаёт tenant
domain contract и не должен растворяться в middleware- или host-specific логике.

## Назначение

- публиковать канонический tenant domain contract и CRUD/module-toggle surfaces;
- держать tenant-aware domain logic внутри модуля;
- удерживать `apps/server` в роли middleware/cache adapter слоя, а не владельца tenancy domain.

## Зона ответственности

- tenant и tenant-module entities/DTOs/services;
- public CRUD, module enablement и tenant settings contract;
- schema guard для tenant settings (object JSON + depth/key/payload limits);
- transactional outbox-публикация tenant lifecycle events (`tenant.created`, `tenant.updated`, `tenant.module.toggled`) при wiring `TenantService` с `TransactionalEventBus`;
- tenant-scoped business rules, которые потребляют остальные модули платформы;
- инварианты multi-tenant модели: `tenant_id`, tenant filtering и tenant-scoped module enablement.

## Интеграция

- `apps/server` владеет только middleware resolution entry point, cache infrastructure и runtime bootstrap вокруг tenant resolver path;
- tenant context разрешается по `uuid`, `slug` или `host` до входа в бизнес-логику;
- outbox relay/dispatch инфраструктура остаётся host/runtime concern, но `rustok-tenant` должен публиковать tenant lifecycle events через `TransactionalEventBus` без локальных bypass;
- tenant admin read paths должны проходить через tenant-scoped RBAC checks (`tenants:(read|list|manage)` + `modules:(read|list|manage)`) и оставаться синхронизированными с server adapters;
- Redis/in-memory cache semantics и cross-instance invalidation принадлежат host cache layer, но должны оставаться синхронизированными с module contract;
- observability для tenant runtime публикуется host-слоем через `/metrics`, включая cache hit/miss, coalesced requests и active/inactive tenant signals;
- любые tenant-scoped runtime guarantees требуют синхронизации module docs и server docs.

## Проверка

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- targeted tests для tenant CRUD, module toggles, resolver invariants и cache-aware integration path

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Server docs](../../../apps/server/docs/README.md)
- [Cache stampede protection](../../../apps/server/docs/CACHE_STAMPEDE_PROTECTION.md)
