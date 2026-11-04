# Implementation plan for `rustok-region`

## Scope of work

- preserve region-owned country, currency, and tax-provider policy;
- maintain module-owned admin and storefront UI boundaries;
- collect runtime evidence before FBA status promotion.

## Current state

`rustok-region` owns regions, countries, currencies, and the baseline regional
tax-provider policy. `region.tax_provider_id` is the canonical provider field;
an explicit channel override map is supported only for channel-aware cart
runtime. This module does not own tenant locale policy or the full tax domain.

Both admin and storefront packages are module-owned. Native adapters use
`HostRuntimeContext`; storefront preserves its selected GraphQL path. Core owns
route/query, presentation, validation, and typed error evidence, while UI
adapters bind and render prepared state.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `RegionReadPort` / `region.read_projection.v1` in
  `crates/modules/rustok-region/contracts/region-fba-registry.json`.
- Owner read-port error safety: `source_closed_unvalidated`; public validation
  messages and owner diagnostics are bounded by
  `crates/modules/rustok-region/docs/region-owner-port-error-safety.md` and
  `scripts/verify/verify-region-owner-port-error-safety.mjs` without promoting
  FFA/FBA status.
- Static and runtime-order evidence:
  `crates/modules/rustok-region/contracts/evidence/region-contract-test-static-matrix.json`
  and `crates/modules/rustok-region/contracts/evidence/region-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-region-admin-boundary.mjs`,
  `scripts/verify/verify-region-storefront-boundary.mjs`, and
  `npm run verify:region:fba` lock the owner UI boundaries and read-provider
  order.
- The admin surface is an accepted single-adapter owner fragment: it is an
  authenticated operator workflow with no public/headless region-admin
  contract, so its native `#[server]` adapter is intentional and no
  package-local GraphQL fallback is added.

## Open results

1. **Collect live RegionReadPort and storefront transport evidence.** Exercise
   shared-context reads plus native success/failure, GraphQL success, and
   double-failure error envelopes before FBA promotion.
   **Depends on:** composed region provider, storefront host, and test tenants.
   **Done when:** tenant scope, locale fallback, typed errors, DOM evidence, and
   selected-path parity are reproducible in runtime tests.

2. **Evolve regional policy through the owner service.** Add country/currency
   rules and tax-provider behavior only as module-owned policy, preserving the
   boundary between baseline region data, explicit channel overrides, and the
   tax calculation domain.
   **Depends on:** region requirements, tax provider contracts, and commerce
   store-context consumers.
   **Done when:** policy selection is deterministic, tested for edge cases, and
   cart/order consumers retain the selected provider snapshot.

3. **Keep module and commerce documentation synchronized.** Update local,
   admin, storefront, manifest, and umbrella commerce docs with a region or
   transport contract change.
   **Depends on:** the change-owning regional or commerce contract.
   **Done when:** route ownership, fallback behavior, and tax-provider policy
   describe the same module boundary everywhere.

4. **Validate the Region owner-port error-safety source contract.** Execute the
   focused verifier, compile the Region and Commerce consumers, and retain
   mounted failure-envelope evidence without exposing raw owner payloads.
   **Depends on:** repository checkout and composed Region consumers.
   **Done when:** source guard, compile, and mounted runtime evidence are retained
   without changing the current FFA/FBA status prematurely.

## Scaffold reference candidate

The Region create/read slice is the candidate with owner storage and no Fly
contribution. Its [file map](README.md#region-command-and-query-file-map)
documents the service split. The SQLite and PostgreSQL create/read tests apply
the owner migration list and verify country resolution, locale fallback, and
tenant isolation. A SQLite fault test inserts a journal entry, injects a
readback failure, and verifies that the entire update rolls back.

Cart tax recalculation now reads `RegionTaxPolicy` through the native
`RegionService::read_tax_policy_in_tx` entrypoint. The private Region mappings
live inside this crate. Cart and Commerce fixtures apply the owner migrations
and create Regions through the service. The
[ownership decision](../../../../DECISIONS/2026-11-04-region-storage-and-cart-tax-policy.md)
records the transaction and deployment profile. Complete the remaining layout
checks and verify the declared admin and storefront clients before enrolling
the complete module.

## Verification

- `node scripts/verify/verify-region-owner-port-error-safety.mjs`
- `npm run verify:region:admin-boundary`
- `npm run verify:region:storefront-boundary`
- `npm run verify:region:fba`
- `cargo xtask module validate region`
- `cargo xtask module test region`
- `cargo test -p rustok-region --lib --test region_service_test --test module`
- `cargo test -p rustok-region --test region_service_test postgres_create_and_resolve_region_by_country -- --ignored`
  runs the same create/read scenario in a fresh database. Set
  `RUSTOK_REGION_TEST_POSTGRES_URL` to a test PostgreSQL instance with database
  creation permission. The fixture removes the database after the run. The
  Region PostgreSQL workflow runs the same crate-scoped scenario.
- Targeted region lookup, country/currency policy, tax-baseline, and storefront
  transport tests.

On 2026-11-04, on local `main` at `111d6dd4bf` with the Region slice changes
applied, Cargo passed five crate and module contract tests and ten SQLite
tests. The Cargo-built integration test binary passed the same create/read
scenario on PostgreSQL 16.13. The Region FBA and owner-port source guards
passed. The Commerce GraphQL error-safety verifier still expects an earlier
adapter shape and needs separate reconciliation with Commerce's current code.

## Update rules

1. Keep regional policy and region read projections in this module.
2. Update local docs, `rustok-module.toml`, and commerce/tax documentation with
   a regional or transport contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
