# rustok-tenant

## Purpose

`rustok-tenant` owns tenant lifecycle and per-tenant module enablement for RusToK.

## Responsibilities

- Provide `TenantModule` metadata for the runtime registry.
- Manage tenant CRUD and module toggle state.
- Publish the typed `tenants:*` and `modules:*` RBAC surface.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Used by `apps/server` tenant middleware, tenant admin flows, and module toggle orchestration.
- Exposes a module-owned Leptos admin overview through `rustok-tenant-admin`.
- Declares permissions via `rustok-core::Permission`.
- `apps/server` enforces those permissions through `RbacService` and GraphQL/REST RBAC guards.
- Module lifecycle orchestration lives in `apps/server`, while `rustok-tenant` owns the
  tenant-side state and DTO contracts.

## Entry points

- `TenantModule`
- `TenantService`
- `CreateTenantInput`
- `UpdateTenantInput`
- `ToggleModuleInput`
