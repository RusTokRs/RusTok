# Implementation plan for `rustok-tenant`

## Current state

`rustok-tenant` owns the tenant domain, tenant-module contract, validation,
tenant lifecycle events, and the `TenantReadPort` read projection. The server
owns resolver middleware, cache infrastructure, provisioning orchestration, and
runtime composition; it must not take over tenant business rules.

The host cache-miss resolver and installer provisioning/verification use
`TenantReadPort` for typed id, slug, and domain reads. Idempotent installer
tenant provisioning uses `TenantService::ensure_tenant`, so the host does not
duplicate tenant persistence semantics. The module keeps inactive tenants hidden
unless explicitly requested and requires read deadlines. Cache invalidation after
lifecycle changes remains a server-owned integration responsibility.

Tenant resolution and the separate byte-weighted tenant-locale cache share the
durable tenant generation channel. In-order records invalidate the exact tenant
locale entry, namespace-wide manual rotations carry `*`, and unverified, gapped,
lagged or reconciled advancement clears every process-local locale entry before
acknowledgement. Every event is checked against the durable generation before
cache mutation or tracker acknowledgement. When durable state is already ahead
of an otherwise in-order exact event, the listener treats the difference as a
missed invalidation, full-clears the locale namespace and records the durable
offset instead of applying only the event key. Generation regression clears local
locale values but remains fail-closed; it never lowers the trusted process epoch.
The listener exposes recovery health, not only task liveness, to the critical
runtime guardrail.

Source evidence now covers:

- two independent serving contexts whose real `content-language` values prove
  exact UUID invalidation leaves another tenant cached until a later wildcard
  clear;
- deterministic overflow of the 256-message local queue with two serving
  listeners and a probe, followed by durable recovery of both locale values;
- a durable `N+2` state observed while an exact `N+1` event is received, proving
  both tenant entries are rebuilt and the tracker advances directly to `N+2`;
- a completely missed Redis publication recovered by the same periodic
  reconciliation loop with a shortened test interval;
- Redis stop/restart with lost generation state, stale-value clearing while
  readiness remains failed, explicit restoration of the previous epoch, and
  successful delivery of the next `N+1` event to the original replicas;
- permanent workflow and source guards for path scope, compiled tests, ignored
  Redis tests, durable-before-apply ordering, durable-ahead full recovery and the
  prohibition on tracker rebaselining after regression.

This evidence is source-complete but is not compiled or live verified on the
current revision until the cache workflow reports successful compiled and Redis
jobs.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `transport_verified`
- Structural shape: `core_transport_ui`
- FBA provider contract: `TenantReadPort` / `tenant.read_projection.v1` in
  `crates/rustok-tenant/contracts/tenant-fba-registry.json`.
- Static and runtime evidence:
  `crates/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json`
  and `crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json`.
- The admin overview is a documented native-only exception: no public
  GraphQL/REST tenant-bootstrap UI contract exists yet. Its native adapter uses
  `HostRuntimeContext` and is host-neutral.

## Open results

1. **Execute multi-replica tenant-locale recovery evidence.** Run exact UUID,
   wildcard, durable-ahead gap recovery, deterministic lag, missed-publication
   reconciliation and Redis state-loss/restoration scenarios on the same
   reconciled `main` revision.
   **Depends on:** the permanent cache workflow or another Rust 1.96 environment
   with isolated Redis 7 and `redis-server`.
   **Done when:** compiled and live Redis jobs pass on one revision, every failure
   is fixed, and the verified revision is recorded without copying raw logs.

2. **Collect deployed parity evidence for the native tenant overview.** Confirm
   host locale, tenant-scoped RBAC, disabled/not-found behavior, and typed error
   mapping in the composed runtime before any FFA-status promotion.
   **Depends on:** a deployed host with representative tenant identities.
   **Done when:** the result is reproducible and the native-only exemption is
   either retained with evidence or replaced by an owned public contract.

3. **Keep lifecycle and cache behavior synchronized.** Any change to create,
   update, deactivate, domain, locale, or module-toggle behavior must preserve
   typed `TenantReadPort` use and the shared durable generation contract for UUID,
   slug, host and locale cache views.
   **Depends on:** the server resolver/cache adapters and
   `ModuleLifecycleService`.
   **Done when:** targeted resolver/invalidation tests cover the changed state
   transition and no production path calls the low-level legacy toggle helper.

4. **Maintain FBA read-projection compatibility.** Evolve selector, deadline,
   inactive-tenant, or error semantics atomically across provider, server
   resolver, installer, metadata, and evidence.
   **Depends on:** all registered `TenantReadPort` consumers.
   **Done when:** runtime evidence and consumer metadata agree on the published
   `tenant.read_projection.v1` contract.

## Verification

- Contract tests cover every public use case.
- `npm run verify:tenant:fba`
- `npm run verify:tenant:admin-boundary`
- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant tenant_read_port --test integration`
- `cargo test -p rustok-server --test tenant_locale_generation_guard`
- `cargo test -p rustok-server tenant_locale_generation --lib`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server cargo test -p rustok-server tenant_locale_generation --lib -- --ignored --nocapture --test-threads=1`

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)

## Change rules

1. Keep tenancy business logic and `TenantReadPort` in this module.
2. Update the local README, `rustok-module.toml`, and server documentation with
   a public/runtime contract change.
3. Update this status block and `docs/modules/registry.md` with a UI or
   transport boundary change.
