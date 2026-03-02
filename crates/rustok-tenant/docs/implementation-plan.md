# rustok-tenant module implementation plan (`rustok-tenant`)

## Scope and objective

This document captures the current implementation plan for `rustok-tenant` in RusToK and
serves as the source of truth for rollout sequencing in `crates/rustok-tenant`.

Primary objective: evolve `rustok-tenant` in small, testable increments while preserving
compatibility with platform-level contracts.

## Target architecture

- `rustok-tenant` remains focused on its bounded context and public crate API.
- Integrations with other modules go through stable interfaces in `rustok-core`
  (or dedicated integration crates where applicable).
- Behavior changes are introduced through additive, backward-compatible steps.
- Observability and operability requirements are part of delivery readiness.

## Delivery phases

### Phase 0 — Foundation (done)

- [x] Baseline crate/module structure is in place.
- [x] Base docs and registry presence are established.
- [x] Core compile-time integration with the workspace is available.

### Phase 1 — Contract hardening (done)

- [x] SeaORM entities implemented: `tenant.rs` (id, name, slug, domain, settings, is_active, timestamps) and `tenant_module.rs` (id, tenant_id, module_slug, enabled, settings, timestamps).
- [x] DTOs implemented: `CreateTenantInput`, `UpdateTenantInput`, `TenantResponse`, `TenantModuleResponse`, `ToggleModuleInput`.
- [x] `TenantService` implemented with methods: `create_tenant()`, `get_tenant()`, `get_tenant_by_slug()`, `update_tenant()`, `list_tenants()`, `toggle_module()`, `list_tenant_modules()`.
- [x] Error types: `TenantError` with `SlugAlreadyExists`, `NotFound`, `Database` variants via `thiserror`.
- [x] Public API re-exported from `lib.rs`: all DTOs and `TenantService`.
- [x] `TenantModule::health()` returns `HealthStatus::Healthy`.
- [x] Migrations managed by `apps/server/migration` (tenants + tenant_modules tables).

### Phase 2 — Domain expansion (planned)

- [ ] Tenant settings schema validation (JSON Schema or validator-based).
- [ ] Tenant domain/subdomain resolution integration with middleware.
- [ ] Events: `TenantCreated`, `TenantUpdated`, `TenantModuleToggled` via outbox.
- [ ] RBAC: tenant-scoped admin permissions.

### Phase 3 — Productionization (planned)

- [ ] Integration tests for `TenantService` CRUD and toggle operations.
- [ ] Audit trail for tenant configuration changes.
- [ ] Observability: metrics for tenant cache hit/miss, active tenants count.
- [ ] Runbook for tenant provisioning and deprovisioning.

## Tracking and updates

When updating `rustok-tenant` architecture, API contracts, tenancy behavior, routing,
or observability expectations:

1. Update this file first.
2. Update `crates/rustok-tenant/README.md` and `crates/rustok-tenant/docs/README.md` when public behavior changes.
3. Update `docs/index.md` links if documentation structure changes.
4. If module responsibilities change, update `docs/modules/registry.md` accordingly.
