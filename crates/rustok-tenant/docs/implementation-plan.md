# Implementation plan for `rustok-tenant`

Status: base tenant domain contract is stabilized; the current iteration
is moved to planning the next tenant-domain increment.

## Execution checkpoint

- Current phase: iteration_2_fba_transport_verified
- Last checkpoint: installer provisioning/verification now consumes module-owned `TenantReadPort` for slug read projections before create-candidate decisions and final verification, while host-owned mutation/module lifecycle flows remain in `apps/server`; static FBA evidence source-locks resolver and installer handoffs and no long compilation was run in this iteration.
- Next step: Execute authored runtime smoke when compilation is allowed and keep FFA parity/evidence hardening for the native-only overview surface instead of mechanical UI expansion.
- Open blockers: None.
- Hand-off notes for next agent: Do not expand scope to a new tenant feature set; in this iteration keep focus on lifecycle consistency and regression safety between the module and host middleware/cache path.
- Last updated at (UTC): 2026-06-30T20:14:10Z

## Scope of work

- maintain `rustok-tenant` as owner of the tenant domain contract;
- synchronize tenancy invariants, resolver expectations and local docs;
- expand tenancy surface without shifting business logic into `apps/server`.

## Current state

- entities `tenants` and `tenant_modules`, DTO and `TenantService` are already implemented;
- tenant middleware resolution and cache infrastructure remain host-owned integration path;
- module enablement is already locked as a tenant-scoped contract;
- root `README.md`, local docs and manifest metadata are part of the scoped module audit.

## Stages

### 1. Contract stability

- [x] lock base tenant CRUD and module-toggle contract;
- [x] lock responsibility split between the module and server middleware/cache layer;
- [x] maintain sync between tenancy invariants, server resolver path and module metadata.

### 2. Domain expansion

- [x] add schema validation for tenant settings (object-only JSON, depth/key/payload limits);
- [x] deliver outbox events for `TenantCreated`, `TenantUpdated`, `TenantModuleToggled` (through `TransactionalEventBus` in tenant mutation flows);
- [x] synchronize tenancy contract with RBAC for tenant-scoped admin permissions (tenant admin bootstrap + server GraphQL tenant/module read paths aligned with `modules:(read|list|manage)` and `tenants:(read|list|manage)` checks).

### 3. Operability

- [x] deliver integration tests for tenant CRUD, module toggles and resolver invariants (baseline CRUD/module-toggle/outbox tests in `crates/rustok-tenant/tests/integration.rs`, resolver invariants in `apps/server/tests/tenant_resolver_invariants_test.rs`);
- [x] develop observability for cache hit/miss and active tenant signals (Prometheus surface supplemented with `rustok_tenant_cache_coalesced_requests` + `rustok_tenant_(active|inactive|total)_total`);
- [x] document provisioning/deprovisioning and invalidation guarantees concurrently with runtime contract changes.

### 4. Iteration 2 — tenant lifecycle hardening

- [x] add integration coverage for host provisioning/deprovisioning path: after create/update/deactivate/domain-change, invalidation hooks `invalidate_tenant_cache_by_uuid/slug/host` are verified (server resolver regression tests now cover stale positive cache after deactivate/update, negative cache after create-like flow, host cache after domain-change and UUID invalidation);
- [x] expand server resolver regression matrix for lifecycle invalidation (positive + negative cache scenarios after tenant state transition);
- [x] expand `TenantReadPort` selector surface for host/provisioning consumers: id, slug and domain lookups use a single typed `PortContext`/`PortError` contract, blank slug/domain selectors return validation errors, inactive tenants remain hidden unless `include_inactive=true`;
- [x] lock migration note on deprecated `TenantService::toggle_module`: runtime module enable/disable path must go through host `ModuleLifecycleService` (`README.md`, `docs/README.md` and this plan are synchronized; legacy method kept only as low-level/backfill test helper).
- [x] connect host resolver cache-miss path to `TenantReadPort`: `apps/server` builds typed id/slug/domain requests, requires read deadline semantics through `PortContext`, maps inactive projection to forbidden negative cache, not-found to not-found negative cache and preserves cache coalescing/invalidation in host layer.
- [x] connect installer provisioning/verification read-facts path to `TenantReadPort`: `apps/server/src/installer_cli.rs` now uses slug read projection with `PortContext` deadline semantics for seed inspection and verify step; `NotFound` is treated only as create-candidate, other `PortError` values propagate with typed code/message, while mutation/module lifecycle orchestration remains host-owned.


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `transport_verified`
- Structural shape: `core_transport_ui`
- FBA transport evidence: `cargo test -p rustok-tenant tenant_read_port --test integration` passed 3/3 on 2026-06-30, covering deadline enforcement, typed validation/not-found mapping, id/slug/domain active projection parity, inactive hidden mode and explicit `include_inactive` recovery.
- Evidence: admin UI split now follows the FFA shape: `admin/src/core.rs` owns Leptos-free tenant bootstrap view-model/copy/error policy, `admin/src/transport/mod.rs` owns the module transport facade, `admin/src/transport/native_server_adapter.rs` contains the native server function endpoint, and `admin/src/ui/leptos.rs` is the explicit Leptos render adapter. Fast guardrail coverage now includes `scripts/verify/verify-tenant-admin-boundary.mjs` plus `scripts/verify/verify-tenant-admin-boundary.test.mjs` fixture regressions for canonical split, removed `api.rs`, Leptos-free core, UI facade-only transport calls and server-function adapter placement. FBA provider metadata now exposes the tenant read-projection boundary through `TenantReadPort` / `tenant.read_projection.v1`: `crates/rustok-tenant/contracts/tenant-fba-registry.json`, `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json`, `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json` and `scripts/verify/verify-tenant-fba.mjs` lock shared `rustok_api::PortContext`/`PortError` usage, `PortCallPolicy::read()` deadline semantics, inactive-tenant degraded-mode semantics, server-host/server-installer consumer metadata, the host resolver handoff in `apps/server/src/middleware/tenant.rs` and installer provisioning handoff in `apps/server/src/installer_cli.rs`. No-compile runtime fallback smoke is additionally locked by `npm run verify:foundation:fba-runtime-smoke`; runtime contract/fallback smoke cases in `crates/rustok-tenant/tests/integration.rs` now have compiled runtime evidence for missing deadlines, blank slug/domain validation, id/slug/domain active projection parity, inactive hidden mode and explicit `include_inactive` recovery. FBA status is `transport_verified`.
- Temporary parity note: the current tenant admin overview remains a native-only single-adapter state because there is no legacy GraphQL/REST tenant bootstrap UI contract to preserve for this surface; the existing server GraphQL tenant/module read paths remain unchanged outside this UI package.

## Verification

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant --tests`
- `cargo test -p rustok-server --test tenant_resolver_invariants_test`
- `npm run verify:tenant:fba`
- `npm run verify:foundation:fba-runtime-smoke`
- `node --check scripts/verify/verify-tenant-fba.mjs`
- `cargo test -p rustok-tenant tenant_read_port --test integration` (passed 3/3 on 2026-06-30; includes id/slug/domain selector coverage)
- targeted tests for CRUD, module toggles, resolver invariants and cache integration path, including lifecycle invalidation scenarios `slug_cache_invalidation_refreshes_deactivated_tenant_state`, `slug_negative_cache_invalidation_allows_created_tenant_to_resolve`, `host_cache_invalidation_refreshes_domain_change`, `uuid_cache_invalidation_refreshes_updated_tenant_state`
- contract tests cover all public use-cases, including tenant CRUD, module toggles and resolver-facing invariants

## Update rules

1. When changing tenancy contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing resolver/cache expectations, also update server docs.


## Quality backlog

- [x] Update test coverage for key module scenarios.
- [x] Verify completeness and accuracy of `README.md` and local docs.
- [x] Lock/update verification gates for current module state.
- [x] Add lifecycle-focused integration checks for cache invalidation after tenant state transitions.
