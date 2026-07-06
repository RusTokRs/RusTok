# Implementation plan for `rustok-tax`

Status: FBA provider boundary in progress.

## Execution checkpoint

- Current phase: fba_provider_static_evidence
- Last checkpoint: Tax calculation provider boundary now has a neutral `TaxCalculationPort`, module metadata, machine-readable registry and static evidence verified by `npm run verify:tax:fba`.
- Next step: Replace static contract evidence with runtime contract execution and fallback smoke before any `boundary_ready` promotion.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-06-18T00:00:00Z


## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Evidence:
  - batch no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` and fixture-regression suite check `crates/rustok-tax/contracts/evidence/tax-runtime-contract-smoke.json`: shared read policy precedes owner `TaxService`, typed error mapping and fallback/degraded registry parity are protected from drift; status remains `in_progress` until live provider execution;
  - FBA provider registry `crates/rustok-tax/contracts/tax-fba-registry.json`, static contract evidence `crates/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json` and neutral `TaxCalculationPort`/`tax.calculation.v1` are locked for cart tax calculation consumers; runtime contract execution/fallback smoke remain pending before `boundary_ready`;
  - `scripts/verify/verify-tax-fba.mjs` checks manifest metadata, local/central plan sync, typed `PortContext`/`PortError`, in-process `TaxService` implementation, serializable tax DTOs and static evidence drift.
- Last verified at (UTC): 2026-06-18T00:00:00Z
- Owner: `rustok-tax` module team

## Goal

- move tax calculation from hardcoded cart runtime into a separate bounded context;
- lock the provider seam before real external integrations;
- make `provider_id` part of the tax snapshot contract.

## Current state

- default provider `region_default` retains the current region-based tax policy;
- `rustok-cart` calls `TaxService` rather than calculating tax directly from `region`;
- current provider selection hook lives in `regions.tax_provider_id`;
- cart/order tax lines receive typed `provider_id`.

## Next steps

- tax rules beyond flat region rate;
- provider registry and external engine adapters;
- richer jurisdiction metadata and transport parity tests.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
