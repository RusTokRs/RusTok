# `rustok-tenant` Documentation

`rustok-tenant` — canonical tenancy module of the platform. It defines the tenant
domain contract and must not dissolve into middleware or host-specific logic.

## Purpose

- publish the canonical tenant domain contract, CRUD and tenant-module read surfaces;
- keep tenant-aware domain logic inside the module;
- keep `apps/server` in the middleware/cache adapter layer role, not as owner of the tenancy domain.

## Scope

- tenant and tenant-module entities/DTOs/read services;
- public tenant CRUD, module-state projections and tenant settings contract;
- schema guard for tenant settings (object JSON + depth/key/payload limits);
- mandatory transactional outbox publication of tenant lifecycle and locale-policy
  events from every owner mutation, including installer/bootstrap tenant creation;
- concurrent-idempotent `ensure_tenant`: a losing unique-slug insert re-reads and
  returns the committed tenant instead of leaking a DB error to installer/bootstrap;
- incremental locale-policy migration that seeds one enabled default row for any
  legacy tenant created before `tenant_locales` existed, before new invariants are installed;
- cross-backend locale-policy constraints: PostgreSQL and SQLite use filtered unique
  indexes, while MySQL uses a generated nullable tenant UUID plus a unique index so
  every supported backend rejects multiple default locales for one tenant;
- tenant-scoped business rules consumed by other platform modules;
- invariants of the multi-tenant model: `tenant_id`, tenant filtering and tenant-scoped module enablement.

## Integration

- `apps/server` owns only the middleware resolution entry point, cache infrastructure and runtime bootstrap around the tenant resolver path;
- tenant context is resolved by `uuid`, `slug` or `host` before entering business logic; the module-owned `TenantReadPort` covers read projection lookup by id/slug/domain for host resolver/provisioning consumers; `apps/server` resolver uses this port on the cache-miss path instead of raw entity lookup, and installer provisioning/verification uses slug projection before create-candidate decisions and verify step;
- outbox relay/dispatch infrastructure remains a host/runtime concern, but `rustok-tenant` always inserts each owner lifecycle event through `TransactionalEventBus::publish_root_in_tx` before the state transaction commits;
- `TenantService::new` is the only service constructor; lifecycle publication cannot be disabled by omitted host wiring;
- installer/bootstrap callers delegate tenant creation to `ensure_tenant`; its owner-side replay keeps independent provisioning calls idempotent without host-local retries;
- tenant admin read paths must go through tenant-scoped RBAC checks (`tenants:(read|list|manage)` + `modules:(read|list|manage)`) and remain synchronized with server adapters;
- tenant admin native server-function transport consumes host-provided `rustok_api::HostRuntimeContext` for DB access and must not import a host-wide `AppContext`;
- Redis/in-memory cache semantics and cross-instance invalidation belong to the host cache layer, but must remain synchronized with the module contract;
- host provisioning/deprovisioning flows must call tenant cache invalidation hooks (`invalidate_tenant_cache_by_uuid/slug/host`) after create/update/deactivate/domain-change; without this, stale positive cache may live up to `TENANT_CACHE_TTL=300s`, and negative cache miss up to `TENANT_NEGATIVE_CACHE_TTL=60s`;
- runtime enable/disable of modules must go through `ModuleLifecycleService` / `ModuleControlPlane`, which perform policy/dependency checks, lifecycle hooks and journaling; `TenantService` has no public low-level module-state writer;
- locale-policy writes use revision CAS plus durable idempotency receipts; the public owner port performs one bounded retry after a revision conflict so a concurrent identical request can replay the committed receipt instead of surfacing a stale-revision error;
- the retained PostgreSQL locale race uses two independent owner connections and an explicit tenant-row lock barrier, then proves one revision, one receipt and one locale event set survive the identical-key race while different payload reuse remains a typed conflict;
- the retained PostgreSQL ensure race uses an advisory-lock insert trigger so both owner connections pass the absence read before one unique winner commits; both calls return the same tenant while only one locale seed and one `tenant.created` event persist;
- resolver invariants in the host middleware integration path are captured by tests in `apps/server/tests/tenant_resolver_invariants_test.rs` (header/host/subdomain + disabled/not-found semantics);
- observability for tenant runtime is published by the host layer via `/metrics`, including cache hit/miss, coalesced requests and active/inactive tenant signals;
- any tenant-scoped runtime guarantees require synchronization of module docs and server docs.

## Verification

- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `npm run verify:tenant:fba`
- `node --check scripts/verify/verify-tenant-fba.mjs`
- `node scripts/verify/verify-tenant-locale-policy-migration.mjs`
- PostgreSQL backfill fixture `tenant-locale-policy-invariants` from `docs/migrations/backfill-contracts.json`
- MySQL migration execution proving `default_tenant_guard` rejects a second default locale for the same tenant
- `cargo test -p rustok-tenant --test tenant_ensure_concurrency_postgres -- --nocapture`
- `cargo test -p rustok-tenant --test integration tenant_mutations_always_publish_outbox_events -- --nocapture`
- `cargo test -p rustok-tenant tenant_read_port --test integration` for FBA read-port runtime smoke (deadline, typed error mapping, slug/domain lookup, inactive degraded mode)
- `cargo test -p rustok-tenant tenant_locale_policy --test integration` for sequential CAS, durable receipt replay and key-reuse conflicts
- `cargo test -p rustok-tenant --test locale_policy_concurrency_postgres -- --nocapture` for the retained two-connection PostgreSQL idempotency/CAS race; it uses `RUSTOK_TENANT_TEST_DATABASE_URL` or `DATABASE_URL`
- server lifecycle tests proving all runtime module-state writes use the control plane
- targeted tests for tenant CRUD, resolver invariants and cache-aware integration path

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Server docs](../../../apps/server/docs/README.md)
- [Cache stampede protection](../../../apps/server/docs/CACHE_STAMPEDE_PROTECTION.md)
