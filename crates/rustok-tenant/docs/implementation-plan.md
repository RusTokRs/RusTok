# Implementation plan for `rustok-tenant`

## Current state

`rustok-tenant` owns the tenant domain, tenant-module contract, validation,
tenant lifecycle events, and the `TenantReadPort` read projection. The server
owns resolver middleware, cache infrastructure, provisioning orchestration, and
runtime composition; it must not take over tenant business rules.

The host cache-miss resolver and installer provisioning/verification use
`TenantReadPort` for typed id, slug, and domain reads. The module keeps inactive
tenants hidden unless explicitly requested and requires read deadlines. Cache
invalidation after lifecycle changes remains a server-owned integration
responsibility.

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
  `HostRuntimeContext` and is Loco-free.

## Open results

1. **Collect deployed parity evidence for the native tenant overview.** Confirm
   host locale, tenant-scoped RBAC, disabled/not-found behavior, and typed error
   mapping in the composed runtime before any FFA-status promotion.
   **Depends on:** a deployed host with representative tenant identities.
   **Done when:** the result is reproducible and the native-only exemption is
   either retained with evidence or replaced by an owned public contract.

2. **Keep lifecycle and cache behavior synchronized.** Any change to create,
   update, deactivate, domain, or module-toggle behavior must preserve typed
   `TenantReadPort` use and invalidate the relevant UUID, slug, and host cache
   keys.
   **Depends on:** the server resolver/cache adapter and
   `ModuleLifecycleService`.
   **Done when:** targeted resolver/invalidation tests cover the changed state
   transition and no production path calls the low-level legacy toggle helper.

3. **Maintain FBA read-projection compatibility.** Evolve selector, deadline,
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
- Targeted server resolver and cache-invalidation tests.

## Change rules

1. Keep tenancy business logic and `TenantReadPort` in this module.
2. Update the local README, `rustok-module.toml`, and server documentation with
   a public/runtime contract change.
3. Update this status block and `docs/modules/registry.md` with a UI or
   transport boundary change.
