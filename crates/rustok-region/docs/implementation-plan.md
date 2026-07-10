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
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- FBA provider contract: `RegionReadPort` / `region.read_projection.v1` in
  `crates/rustok-region/contracts/region-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/rustok-region/contracts/evidence/region-contract-test-static-matrix.json`
  and `crates/rustok-region/contracts/evidence/region-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-region-admin-boundary.mjs`,
  `scripts/verify/verify-region-storefront-boundary.mjs`, and
  `npm run verify:region:fba` lock the owner UI boundaries and read-provider
  order.
- The admin surface is an accepted single-adapter owner fragment: it is an
  authenticated operator workflow with no public/headless region-admin
  contract, so its native `#[server]` adapter is intentional and no
  package-local GraphQL fallback is added.

## Stages

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

## Verification

- `npm run verify:region:admin-boundary`
- `npm run verify:region:storefront-boundary`
- `npm run verify:region:fba`
- `cargo xtask module validate region`
- `cargo xtask module test region`
- Targeted region lookup, country/currency policy, tax-baseline, and storefront
  transport tests.

## Update rules

1. Keep regional policy and region read projections in this module.
2. Update local docs, `rustok-module.toml`, and commerce/tax documentation with
   a regional or transport contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
