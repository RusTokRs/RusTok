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

Tenant resolution and the separate byte-weighted tenant-locale cache now share
the durable tenant generation channel. In-order records invalidate the exact
tenant locale entry, namespace-wide manual rotations carry `*`, and unverified,
gapped, lagged or reconciled advancement clears every process-local locale entry
before acknowledgement. The listener is context-owned, restartable and surfaced
as a critical runtime guardrail when shared Redis delivery is required.

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

1. **Execute multi-replica tenant-locale recovery evidence.** Exercise exact UUID
   invalidation, namespace-wide `*` rotation, local broadcast lag, Redis subscriber
   reconnect, missed-generation reconciliation and configured-but-unavailable
   Redis readiness behavior.
   **Depends on:** a composed multi-replica server runtime and isolated Redis.
   **Done when:** the same durable generation advancement removes stale locale and
   default-locale data on every replica before recovery is acknowledged, and a
   terminal required worker makes readiness non-OK.

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

- `npm run verify:tenant:fba`
- `npm run verify:tenant:admin-boundary`
- `cargo xtask module validate tenant`
- `cargo xtask module test tenant`
- `cargo test -p rustok-tenant tenant_read_port --test integration`
- `cargo test -p rustok-server --test tenant_locale_generation_guard`
- Targeted server resolver, tenant-locale generation, Redis reconnect and
  multi-replica invalidation tests.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)

## Change rules

1. Keep tenancy business logic and `TenantReadPort` in this module.
2. Update the local README, `rustok-module.toml`, and server documentation with
   a public/runtime contract change.
3. Update this status block and `docs/modules/registry.md` with a UI or
   transport boundary change.
