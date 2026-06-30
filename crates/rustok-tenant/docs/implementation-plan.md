# План реализации `rustok-tenant`

Статус: базовый tenant domain contract стабилизирован; текущая итерация
переведена в планирование следующего tenant-domain инкремента.

## Execution checkpoint

- Current phase: iteration_2_fba_transport_verified
- Last checkpoint: installer provisioning/verification now consumes module-owned `TenantReadPort` for slug read projections before create-candidate decisions and final verification, while host-owned mutation/module lifecycle flows remain in `apps/server`; static FBA evidence source-locks resolver and installer handoffs and no long compilation was run in this iteration.
- Next step: Execute authored runtime smoke when compilation is allowed and keep FFA parity/evidence hardening for the native-only overview surface instead of mechanical UI expansion.
- Open blockers: None.
- Hand-off notes for next agent: Не расширять scope на новый tenant feature set; в этой итерации держать фокус на lifecycle consistency и regression safety между модулем и host middleware/cache path.
- Last updated at (UTC): 2026-06-30T20:14:10Z

## Область работ

- удерживать `rustok-tenant` как владельца tenant domain contract;
- синхронизировать tenancy invariants, resolver expectations и local docs;
- расширять tenancy surface без смещения бизнес-логики в `apps/server`.

## Текущее состояние

- сущности `tenants` и `tenant_modules`, DTO и `TenantService` уже реализованы;
- tenant middleware resolution и cache infrastructure остаются host-owned integration path;
- module enablement уже закреплён как tenant-scoped contract;
- root `README.md`, local docs и manifest metadata входят в scoped module audit.

## Этапы

### 1. Contract stability

- [x] закрепить базовый tenant CRUD и module-toggle contract;
- [x] зафиксировать разделение ответственности между модулем и server middleware/cache layer;
- [x] удерживать sync между tenancy invariants, server resolver path и module metadata.

### 2. Domain expansion

- [x] добавить schema validation для tenant settings (object-only JSON, depth/key/payload limits);
- [x] довести outbox events для `TenantCreated`, `TenantUpdated`, `TenantModuleToggled` (через `TransactionalEventBus` в tenant mutation flows);
- [x] синхронизировать tenancy contract с RBAC для tenant-scoped admin permissions (tenant admin bootstrap + server GraphQL tenant/module read paths выровнены по `modules:(read|list|manage)` и `tenants:(read|list|manage)` checks).

### 3. Operability

- [x] довести integration tests для tenant CRUD, module toggles и resolver invariants (baseline CRUD/module-toggle/outbox tests в `crates/rustok-tenant/tests/integration.rs`, resolver invariants в `apps/server/tests/tenant_resolver_invariants_test.rs`);
- [x] развить observability для cache hit/miss и active tenant signals (Prometheus surface дополнен `rustok_tenant_cache_coalesced_requests` + `rustok_tenant_(active|inactive|total)_total`);
- [x] документировать provisioning/deprovisioning и invalidation guarantees одновременно с изменением runtime contract.

### 4. Iteration 2 — tenant lifecycle hardening

- [x] добавить integration coverage для host provisioning/deprovisioning path: после create/update/deactivate/domain-change обязательно проверять invalidation хуков `invalidate_tenant_cache_by_uuid/slug/host` (server resolver regression tests теперь покрывают stale positive cache после deactivate/update, negative cache после create-like flow, host cache после domain-change и UUID invalidation);
- [x] расширить server resolver regression matrix под lifecycle invalidation (positive + negative cache сценарии после tenant state transition);
- [x] расширить `TenantReadPort` selector surface для host/provisioning consumers: id, slug и domain lookup используют единый typed `PortContext`/`PortError` contract, blank slug/domain selectors возвращают validation errors, inactive tenants остаются hidden unless `include_inactive=true`;
- [x] зафиксировать migration note по deprecated `TenantService::toggle_module`: runtime module enable/disable path должен идти через host `ModuleLifecycleService` (`README.md`, `docs/README.md` и этот план синхронизированы; legacy method оставлен только как low-level/backfill test helper).
- [x] подключить host resolver cache-miss path к `TenantReadPort`: `apps/server` строит typed id/slug/domain requests, требует read deadline semantics через `PortContext`, мапит inactive projection в forbidden negative cache, not-found в not-found negative cache и сохраняет cache coalescing/invalidation в host layer.
- [x] подключить installer provisioning/verification read-facts path к `TenantReadPort`: `apps/server/src/installer_cli.rs` теперь использует slug read projection с `PortContext` deadline semantics для seed inspection и verify step; `NotFound` трактуется только как create-candidate, остальные `PortError` всплывают с typed code/message, а mutation/module lifecycle orchestration остаётся host-owned.


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `transport_verified`
- Structural shape: `core_transport_ui`
- FBA transport evidence: `cargo test -p rustok-tenant tenant_read_port --test integration` passed 3/3 on 2026-06-30, covering deadline enforcement, typed validation/not-found mapping, id/slug/domain active projection parity, inactive hidden mode and explicit `include_inactive` recovery.
- Evidence: admin UI split now follows the FFA shape: `admin/src/core.rs` owns Leptos-free tenant bootstrap view-model/copy/error policy, `admin/src/transport/mod.rs` owns the module transport facade, `admin/src/transport/native_server_adapter.rs` contains the native server function endpoint, and `admin/src/ui/leptos.rs` is the explicit Leptos render adapter. Fast guardrail coverage now includes `scripts/verify/verify-tenant-admin-boundary.mjs` plus `scripts/verify/verify-tenant-admin-boundary.test.mjs` fixture regressions for canonical split, removed `api.rs`, Leptos-free core, UI facade-only transport calls and server-function adapter placement. FBA provider metadata now exposes the tenant read-projection boundary through `TenantReadPort` / `tenant.read_projection.v1`: `crates/rustok-tenant/contracts/tenant-fba-registry.json`, `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json`, `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json` and `scripts/verify/verify-tenant-fba.mjs` lock shared `rustok_api::PortContext`/`PortError` usage, `PortCallPolicy::read()` deadline semantics, inactive-tenant degraded-mode semantics, server-host/server-installer consumer metadata, the host resolver handoff in `apps/server/src/middleware/tenant.rs` and installer provisioning handoff in `apps/server/src/installer_cli.rs`. No-compile runtime fallback smoke is additionally locked by `npm run verify:foundation:fba-runtime-smoke`; runtime contract/fallback smoke cases in `crates/rustok-tenant/tests/integration.rs` now have compiled runtime evidence for missing deadlines, blank slug/domain validation, id/slug/domain active projection parity, inactive hidden mode and explicit `include_inactive` recovery. FBA status is `transport_verified`.
- Temporary parity note: the current tenant admin overview remains a native-only single-adapter state because there is no legacy GraphQL/REST tenant bootstrap UI contract to preserve for this surface; the existing server GraphQL tenant/module read paths remain unchanged outside this UI package.

## Проверка

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant --tests`
- `cargo test -p rustok-server --test tenant_resolver_invariants_test`
- `npm run verify:tenant:fba`
- `npm run verify:foundation:fba-runtime-smoke`
- `node --check scripts/verify/verify-tenant-fba.mjs`
- `cargo test -p rustok-tenant tenant_read_port --test integration` (passed 3/3 on 2026-06-30; includes id/slug/domain selector coverage)
- targeted tests для CRUD, module toggles, resolver invariants и cache integration path, включая lifecycle invalidation сценарии `slug_cache_invalidation_refreshes_deactivated_tenant_state`, `slug_negative_cache_invalidation_allows_created_tenant_to_resolve`, `host_cache_invalidation_refreshes_domain_change`, `uuid_cache_invalidation_refreshes_updated_tenant_state`
- контрактные тесты покрывают все публичные use-case, включая tenant CRUD, module toggles и resolver-facing invariants

## Правила обновления

1. При изменении tenancy contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении resolver/cache expectations обновлять также server docs.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.
- [x] Добавить lifecycle-focused integration checks для cache invalidation после tenant state transitions.
