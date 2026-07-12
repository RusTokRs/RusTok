# `rustok-tenant` Documentation

`rustok-tenant` — canonical tenancy module of the platform. It defines the tenant
domain contract and must not dissolve into middleware or host-specific logic.

## Purpose

- publish the canonical tenant domain contract and CRUD/module-toggle surfaces;
- keep tenant-aware domain logic inside the module;
- keep `apps/server` in the middleware/cache adapter layer role, not as owner of the tenancy domain.

## Scope

- tenant and tenant-module entities/DTOs/services;
- public CRUD, legacy low-level module override and tenant settings contract;
- schema guard for tenant settings (object JSON + depth/key/payload limits);
- transactional outbox publication of tenant lifecycle events (`tenant.created`, `tenant.updated`, `tenant.module.toggled`) when wiring `TenantService` with `TransactionalEventBus`;
- tenant-scoped business rules consumed by other platform modules;
- invariants of the multi-tenant model: `tenant_id`, tenant filtering and tenant-scoped module enablement.

## Integration

- `apps/server` owns only the middleware resolution entry point, cache infrastructure and runtime bootstrap around the tenant resolver path;
- tenant context is resolved by `uuid`, `slug` or `host` before entering business logic; the module-owned `TenantReadPort` covers read projection lookup by id/slug/domain for host resolver/provisioning consumers; `apps/server` resolver uses this port on the cache-miss path instead of raw entity lookup, and installer provisioning/verification uses slug projection before create-candidate decisions and verify step;
- outbox relay/dispatch infrastructure remains a host/runtime concern, but `rustok-tenant` must publish tenant lifecycle events through `TransactionalEventBus` without local bypasses;
- tenant admin read paths must go through tenant-scoped RBAC checks (`tenants:(read|list|manage)` + `modules:(read|list|manage)`) and remain synchronized with server adapters;
- tenant admin native server-function transport consumes host-provided `rustok_api::HostRuntimeContext` for DB access and must not import a host-wide `AppContext`;
- Redis/in-memory cache semantics and cross-instance invalidation belong to the host cache layer, but must remain synchronized with the module contract;
- host provisioning/deprovisioning flows must call tenant cache invalidation hooks (`invalidate_tenant_cache_by_uuid/slug/host`) after create/update/deactivate/domain-change; without this, stale positive cache may live up to `TENANT_CACHE_TTL=300s`, and negative cache miss up to `TENANT_NEGATIVE_CACHE_TTL=60s`;
- runtime enable/disable of modules must go through the host `ModuleLifecycleService::toggle_module_with_actor()`: it performs policy/dependency checks, lifecycle hooks and journaling; the deprecated `TenantService::toggle_module` remains only a low-level legacy/backfill API and is not a production entrypoint;
- resolver invariants in the host middleware integration path are captured by tests in `apps/server/tests/tenant_resolver_invariants_test.rs` (header/host/subdomain + disabled/not-found semantics);
- observability for tenant runtime is published by the host layer via `/metrics`, including cache hit/miss, coalesced requests and active/inactive tenant signals;
- any tenant-scoped runtime guarantees require synchronization of module docs and server docs.

## Verification

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `npm run verify:tenant:fba`
- `node --check scripts/verify/verify-tenant-fba.mjs`
- `cargo test -p rustok-tenant tenant_read_port --test integration` for FBA read-port runtime smoke (deadline, typed error mapping, slug/domain lookup, inactive degraded mode)
- targeted tests for tenant CRUD, module toggles, resolver invariants and cache-aware integration path

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Server docs](../../../apps/server/docs/README.md)
- [Cache stampede protection](../../../apps/server/docs/CACHE_STAMPEDE_PROTECTION.md)
