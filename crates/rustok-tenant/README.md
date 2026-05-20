# rustok-tenant

## Purpose

`rustok-tenant` owns tenant lifecycle and per-tenant module enablement for RusToK.

## Responsibilities

- Provide `TenantModule` metadata for the runtime registry.
- Manage tenant CRUD and module toggle state.
- Publish tenant lifecycle events (`tenant.created`, `tenant.updated`, `tenant.module.toggled`) via transactional outbox when `TenantService` is wired with `TransactionalEventBus`.
- Publish the typed `tenants:*` and `modules:*` RBAC surface.
- Keep tenant admin read flows aligned with tenant-scoped RBAC checks for both tenant and module permissions.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Integrates with `rustok-outbox` (`TransactionalEventBus`) to persist tenant lifecycle events transactionally.
- Used by `apps/server` tenant middleware, tenant admin flows, and module toggle orchestration.
- Exposes a module-owned Leptos admin overview through `rustok-tenant-admin`.
- Declares permissions via `rustok-core::Permission`.
- `apps/server` enforces those permissions through `RbacService` and GraphQL/REST RBAC guards.
- Module lifecycle orchestration lives in `apps/server`, while `rustok-tenant` owns the
  tenant-side state and DTO contracts.

## Entry points

- `TenantModule`
- `TenantService` (including `TenantService::with_event_bus` for transactional outbox publishing)
- `CreateTenantInput`
- `UpdateTenantInput`
- `ToggleModuleInput`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
