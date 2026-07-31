# rustok-tenant

## Purpose

`rustok-tenant` owns tenant lifecycle and tenant-scoped module-state projections for RusToK.

## Responsibilities

- Provide `TenantModule` metadata for the runtime registry.
- Manage tenant CRUD, domain lookup, and read-only tenant-module projections.
- Own concurrent-idempotent installer/bootstrap tenant provisioning through
  `TenantService::ensure_tenant`.
- Own the revisioned enabled/default/fallback locale-policy aggregate, including
  canonical tags, exactly one enabled default, valid enabled fallbacks, and
  cycle prevention.
- Publish tenant lifecycle and locale-policy events through the canonical transactional
  outbox in the same owner transaction for every `TenantService` mutation.
- Publish the typed `tenants:*` and `modules:*` RBAC surface.
- Keep tenant admin read flows aligned with tenant-scoped RBAC checks for both tenant and module permissions.
- Keep tenant admin native transport on `rustok_api::HostRuntimeContext`, not a host-wide `AppContext`.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Integrates with `rustok-outbox`; `TenantService` always calls
  `TransactionalEventBus::publish_root_in_tx` before committing owner state.
- Used by `apps/server` tenant middleware, tenant admin flows, installer provisioning,
  and module lifecycle orchestration.
- Tenant resolver invariants for `header`/`host`/`subdomain` resolution and disabled/not-found
  semantics are covered in `apps/server/tests/tenant_resolver_invariants_test.rs`.
- Exposes `TenantReadPort` (`tenant.read_projection.v1`) for transport-neutral read projections by tenant id, slug, or domain with shared `rustok_api::PortContext`/`PortError` deadline semantics.
- Exposes `TenantLocalePolicyPort` for revisioned reads and CAS/idempotent atomic
  replacement. Durable receipts reject reuse of an idempotency key for a
  different request.
- `apps/server` locale middleware consumes `TenantLocalePolicyPort`; it does not
  query `tenant_locales` directly.
- `apps/server` tenant resolver consumes that owner port for cache-miss loads while retaining host-owned cache/coalescing/invalidation concerns.
- Installer/bootstrap calls to `TenantService::ensure_tenant` use the same creation
  transaction and `tenant.created` outbox contract as ordinary tenant creation;
  concurrent unique-slug losers replay the committed tenant.
- Tenant provisioning/deprovisioning flows in the host use `TenantReadPort` for read-fact inspection/verification and are expected to invalidate tenant cache keys
  (`uuid` / `slug` / `host`) to avoid stale resolver state beyond TTL windows.
- Exposes a module-owned Leptos admin overview through `rustok-tenant-admin`.
- Declares permissions via `rustok-core::Permission`.
- `apps/server` enforces those permissions through `RbacService` and GraphQL/REST RBAC guards.
- Runtime module enable/disable is owned by `ModuleLifecycleService` / `ModuleControlPlane`,
  which apply policy, dependency checks, hooks and journaling. `TenantService` exposes
  no low-level module-state writer; only read projections remain public.

## Entry points

- `TenantModule`
- `TenantService`
- `TenantReadPort` / `TenantReadRequest` / `TenantReadSelector`
- `TenantLocalePolicyPort` / `TenantLocalePolicyProjection`
- `ReplaceTenantLocalePolicyRequest`
- `CreateTenantInput`
- `UpdateTenantInput`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
