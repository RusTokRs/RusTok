# Implementation plan for `rustok-telemetry`

Status: telemetry foundation crate already exists, but local documentation and the
boundary contract need to be maintained as rigorously as for other shared modules.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- maintain `rustok-telemetry` as a shared observability foundation layer;
- synchronize telemetry helpers, wiring expectations and local docs;
- do not pull domain-specific observability logic into the foundation crate.

## Current state

- crate is already a shared dependency for observability-related wiring;
- shared telemetry helpers already form part of the platform baseline;
- host and module integrations should rely on a single foundation contract;
- local docs and root `README.md` must remain part of the module-standard path.

## Stages

### 1. Contract stability

- [x] lock `rustok-telemetry` as a shared observability foundation;
- [x] keep shared helpers separate from domain-specific metrics semantics;
- [ ] maintain sync between public surface, host wiring and module metadata.

### 2. Boundary hardening

- [ ] continue extracting shared telemetry helpers from host-specific layers if they are truly shared;
- [ ] do not pull module-owned metrics/runbook semantics here;
- [ ] cover new foundation contracts with targeted tests and compatibility checks;
- [ ] contract tests cover all public use-cases of the telemetry foundation.

### 3. Operability

- [ ] document observability foundation changes concurrently with runtime surface changes;
- [ ] keep local docs and `README.md` synchronized;
- [ ] update host/verification docs if shared wiring expectations change.

## Verification

- `cargo xtask module validate telemetry`
- `cargo xtask module test telemetry`
- targeted tests for telemetry helpers, metrics/tracing wiring and compatibility contracts

## Update rules

1. When changing telemetry foundation contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing shared observability wiring, update related host and verif…997 tokens truncated…che after domain-change and UUID invalidation);
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
- Evidence: admin UI split now follows the FFA shape: `admin/src/core.rs` owns Leptos-free tenant bootstrap view-model/copy/error policy, `admin/src/transport/mod.rs` owns the module transport facade, `admin/src/transport/native_server_adapter.rs` contains the native server function endpoint, and `admin/src/ui/leptos.rs` is the explicit Leptos render adapter. The native endpoint reads `HostRuntimeContext` for DB access and does not import Loco; `crates/rustok-tenant/admin/Cargo.toml` also no longer declares `loco-rs`. Fast guardrail coverage now includes `scripts/verify/verify-tenant-admin-boundary.mjs` plus `scripts/verify/verify-tenant-admin-boundary.test.mjs` fixture regressions for canonical split, removed `api.rs`, Leptos-free core, UI facade-only transport calls, server-function adapter placement and Loco-free native runtime context. FBA provider metadata now exposes the tenant read-projection boundary through `TenantReadPort` / `tenant.read_projection.v1`: `crates/rustok-tenant/contracts/tenant-fba-registry.json`, `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json`, `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json` and `scripts/verify/verify-tenant-fba.mjs` lock shared `rustok_api::PortContext`/`PortError` usage, `PortCallPolicy::read()` deadline semantics, inactive-tenant degraded-mode semantics, server-host/server-installer consumer metadata, the host resolver handoff in `apps/server/src/middleware/tenant.rs` and installer provisioning handoff in `apps/server/src/installer_cli.rs`. No-compile runtime fallback smoke is additionally locked by `npm run verify:foundation:fba-runtime-smoke`; runtime contract/fallback smoke cases in `crates/rustok-tenant/tests/integration.rs` now have compiled runtime evidence for missing deadlines, blank slug/domain validation, id/slug/domain active projection parity, inactive hidden mode and explicit `include_inactive` recovery. FBA status is `transport_verified`.
- Temporary parity note: the current tenant admin overview remains a native-only single-adapter state because there is no legacy GraphQL/REST tenant bootstrap UI contract to preserve for this surface; the existing server GraphQL tenant/module read paths remain unchanged outside this UI package.

## Verification

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant --tests`
- `cargo test -p rustok-server --test tenant_resolver_invariants_test`
- `npm run verify:tenant:fba`
- `npm run verify:foundation:fba-runtime-smoke`
- `node --check scripts/verify/verify-tenant-fba.mjs`
- `node scripts/verify/verify-tenant-admin-boundary.mjs`
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
