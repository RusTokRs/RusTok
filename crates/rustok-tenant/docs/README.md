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
- tenant-scoped business rules, которые потребляют остальные модули платформы;
- инварианты multi-tenant модели: `tenant_id`, tenant filtering и tenant-scoped module enablement.

## Интеграция

- `apps/server` владеет только middleware resolution entry point, cache infrastructure и runtime bootstrap вокруг tenant resolver path;
- tenant context разрешается по `uuid`, `slug` или `host` до входа в бизнес-логику;
- Redis/in-memory cache semantics и cross-instance invalidation принадлежат host cache layer, но должны оставаться синхронизированными с module contract;
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
