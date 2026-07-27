# Implementation plan for `rustok-tax`

## Current state

`rustok-tax` owns tax calculation policy and the neutral `TaxCalculationPort`.
Cart and order receive typed `provider_id` tax snapshots; cart calls
`TaxCalculationPort` rather than implementing region tax logic locally. The default
`region_default` provider preserves current region-based behavior.

This module has no module-owned UI. `calculate_tax` is a read-like port with a
required deadline and typed `PortError` mapping; it must not require write
idempotency.

The canonical root in-process factory now retains the complete delegated
`PortContext` and safe request-shape facts for exact stable validation and
result-invariant outcomes. Policy admission remains owned by the original port
implementation, public errors are unchanged, and direct construction through the
legacy `ports` module remains an explicit compatibility bypass.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- FBA provider contract: `TaxCalculationPort` / `tax.calculation.v1` in
  `crates/rustok-tax/contracts/tax-fba-registry.json`.
- Static and executable no-compile evidence:
  `crates/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json`
  and `crates/rustok-tax/contracts/evidence/tax-runtime-contract-smoke.json`.
- `scripts/verify/verify-tax-fba.mjs` locks provider metadata, root construction,
  port semantics, plan/registry evidence, and fallback metadata.
- `scripts/verify/verify-tax-calculation-policy-context.mjs` and
  `scripts/verify/verify-tax-calculation-local-context.mjs` lock owner policy and
  post-delegation local context retention without promoting runtime evidence.

## Open results

1. **Execute runtime contract and fallback evidence.** Run tax calculation
   through the canonical in-process and remote-adapter profiles before considering
   FBA promotion.
   **Depends on:** cart consumers and a provider runtime environment.
   **Done when:** deadline, typed validation errors, context retention, fallback
   profiles, and provider-id snapshot propagation have executable evidence.

2. **Close compatibility and consumer context gaps.** Audit direct callers of
   `rustok_tax::ports`, migrate production construction to the canonical root
   wrapper, and retain consumer-side transport context across cart/commerce
   boundaries.
   **Depends on:** mounted consumer inventory and transport ownership.
   **Done when:** production callers cannot bypass canonical construction and
   transport diagnostics retain the same request context without raw tax payloads.

3. **Extend tax rules without bypassing the provider boundary.** Add
   jurisdiction metadata and rules beyond flat regional rates through the
   module-owned calculation contract.
   **Depends on:** region policy and a defined jurisdiction data model.
   **Done when:** rule selection is deterministic, serialized snapshots retain
   the selected provider, and cart/order totals agree.

4. **Add external engines through a registry, not cart logic.** Introduce
   provider registration and external adapters only after their failure,
   fallback, and operational ownership are explicit.
   **Depends on:** approved provider integration and operational credentials.
   **Done when:** adapter errors remain typed, no external adapter persists tax
   state, and the recovery procedure is documented.

## Verification

- `node scripts/verify/verify-tax-calculation-local-context.mjs`
- `node scripts/verify/verify-tax-calculation-policy-context.mjs`
- `npm run verify:tax:fba`
- `cargo xtask module validate tax`
- `cargo xtask module test tax`
- Targeted tax calculation, snapshot propagation, and region-policy tests.

## Change rules

1. Keep tax policy and provider selection in this module.
2. Update local documentation, `rustok-module.toml`, and cart/order contracts
   with any calculation or provider change.
3. Update this status block and `docs/modules/registry.md` with an FBA boundary
   change.
