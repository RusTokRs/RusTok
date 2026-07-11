# Implementation plan for `rustok-customer`

## Current state

`rustok-customer` owns tenant-scoped customer records, the optional user/profile
bridge, customer read projections, and the customer admin package. It is not a
replacement for the platform user or public-profile domain. Commerce may
compose customer data through published contracts but must not reintroduce a
customer service facade.

The admin package uses a framework-agnostic core, native transport facade, and
explicit Leptos adapter. Its native server functions use `HostRuntimeContext`;
the old runtime dependency and legacy API facade are removed. Local
documentation is synchronized with the current customer boundary.

The admin surface is an accepted single-adapter owner fragment: it is an
authenticated operator workflow with no public/headless customer-admin contract,
so its native `#[server]` adapter is intentional and no package-local GraphQL
fallback is added.

Local documentation is synchronized.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- FBA provider contract: `CustomerReadPort` / `customer.read_projection.v1` in
  `crates/rustok-customer/contracts/customer-fba-registry.json`.
- Static and source-locked runtime evidence:
  `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json`,
  `crates/rustok-customer/contracts/evidence/customer-runtime-contract-smoke.json`,
  and `crates/rustok-customer/contracts/evidence/customer-read-projection-runtime-smoke.json`.
- `scripts/verify/verify-customer-admin-boundary.mjs` locks the admin boundary;
  `node scripts/verify/verify-customer-fba-no-compile.mjs` locks no-compile
  provider metadata and promotion blockers.

## Open results

0. **Keep profile provisioning enrichment owner-owned.**
   `CustomerReadPort::list_profile_enrichment` exposes only linked user id,
   name components and preferred locale for profile backfill. It must remain a
   narrow projection rather than exposing customer entities or persistence to
   profiles or its future CLI adapter.

1. **Run compiled customer-port evidence.** Execute normalized identity guards,
   tenant-scoped read/list projections, and `PortCallPolicy::read()` deadline
   semantics before considering FBA promotion.
   **Depends on:** a build environment and a runtime-composed consumer.
   **Done when:** targeted customer service/port tests produce runtime evidence
   for every `CustomerReadPort` operation and fallback profile.

2. **Expand settings or profile flows only as customer-owned capabilities.**
   Keep customer settings distinct from auth and public-profile ownership, with
   explicit tenant and optional user/profile bridge rules.
   **Depends on:** a product requirement and the public auth/profile contracts.
   **Done when:** new flows have a module-owned API, tenant-isolation tests, and
   no duplicate policy in auth, profiles, or commerce.

3. **Add diagnostics only for demonstrated operational need.** Tie metrics,
   tracing, or runbook additions to an observed customer lookup, ownership, or
   integration failure mode.
   **Depends on:** production or staging evidence.
   **Done when:** the diagnostic identifies the owning boundary and has an
   actionable recovery procedure.

## Verification

- `npm run verify:customer:admin-boundary`
- `node scripts/verify/verify-customer-fba-no-compile.mjs`
- `npm run verify:ecommerce:fba`
- `cargo xtask module validate customer`
- `cargo xtask module test customer`
- Targeted customer CRUD, identity, ownership, and profile-bridge tests.

## Change rules

1. Keep customer records and their policy in this module.
2. Update local documentation, `rustok-module.toml`, and related auth/profile
   docs when the customer contract changes.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
