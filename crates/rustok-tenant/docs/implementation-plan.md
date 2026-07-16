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

The tenant-resolution cache has shared namespace generation recovery. The
separate tenant-locale cache is byte-weighted and atomically registered, but its
invalidation is currently process-local; another replica may retain a previous
locale/default configuration until the 60-second TTL expires.

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

1. **Approve or replace the tenant-locale cross-replica stale bound.** Decide
   whether the current 60-second TTL is acceptable after locale enablement,
   disablement, ordering, or default-locale changes. Otherwise reserve a
   tenant-locale generation in the committing transaction or consume a persisted
   lifecycle offset on every replica, invalidate before acknowledgement, and
   clear all locale entries on an unverified first event or gap.
   **Depends on:** the locale mutation owner and a durable generation/event
   source; local `invalidate_tenant_locale_cache` calls alone are insufficient.
   **Done when:** the accepted stale bound is explicit and multi-replica evidence
   proves convergence during missed-event and transport-outage scenarios.

2. **Collect deployed parity evidence for the native tenant overview.** Confirm
   host locale, tenant-scoped RBAC, disabled/not-found behavior, and typed error
   mapping in the composed runtime before any FFA-status promotion.
   **Depends on:** a deployed host with representative tenant identities.
   **Done when:** the result is reproducible and the native-only exemption is
   either retained with evidence or replaced by an owned public contract.

3. **Keep lifecycle and cache behavior synchronized.** Any change to create,
   update, deactivate, domain, locale, or module-toggle behavior must preserve
   typed `TenantReadPort` use and invalidate the relevant UUID, slug, host, and
   locale cache keys.
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

- `npm run verify:tenant:fba`
- `npm run verify:tenant:admin-boundary`
- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant tenant_read_port --test integration`
- Targeted server resolver, tenant-locale cache, generation, and multi-replica
  invalidation tests.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)

## Change rules

1. Keep tenancy business logic and `TenantReadPort` in this module.
2. Update the local README, `rustok-module.toml`, and server documentation with
   a public/runtime contract change.
3. Update this status block and `docs/modules/registry.md` with a UI or
   transport boundary change.
