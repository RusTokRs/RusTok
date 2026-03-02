# rustok-tenant

Multi-tenancy module for RusToK: tenant lifecycle, per-tenant module toggles, and metadata management.

## Purpose

`rustok-tenant` is a **Core** platform module responsible for:
- Managing tenant entities (create, read, update, list)
- Per-tenant module enable/disable toggles
- Exposing `TenantService` for use by middleware and other modules

## Responsibilities

- `TenantService` — CRUD for tenants with slug uniqueness validation
- `TenantModule` entity — tracks which modules are enabled per tenant
- SeaORM entities: `tenants` and `tenant_modules` tables
- DTOs: `CreateTenantInput`, `UpdateTenantInput`, `TenantResponse`, `ToggleModuleInput`, `TenantModuleResponse`
- `TenantModule` implements `RusToKModule` with `ModuleKind::Core`

## Interactions

- **Used by**: `apps/server` (tenant resolution middleware, module lifecycle service)
- **Depends on**: `rustok-core` (module trait, SeaORM)
- **Database**: tables `tenants` and `tenant_modules` — managed via `apps/server/migration`

## Entry points

- `crates/rustok-tenant/src/lib.rs` — public API exports
- `crates/rustok-tenant/src/services/tenant_service.rs` — main service logic
- `crates/rustok-tenant/src/entities/` — SeaORM entity definitions

## Documentation

- Local documentation: `./docs/`
- Platform documentation: `/docs/`
- Implementation plan: `./docs/implementation-plan.md`
