# `rustok-tax` Documentation

`rustok-tax` — foundation crate for the tax bounded context in the commerce family.

## Purpose

- typed contract for tax calculation;
- provider seam for future external tax engines;
- default provider `region_default`, which currently preserves the existing semantics
  of `region.tax_rate` / `tax_included`;
- current selection hook via `regions.tax_provider_id`, so that the provider
  choice is already part of the runtime contract before external tax integrations;
- a unified source of truth for `provider_id` in the tax-line snapshot.

## Scope

- the module does not own cart/order transport;
- the module does not own region identity, but consumes a policy snapshot;
- external tax providers must connect over this seam, not directly into
  `rustok-cart` or `rustok-commerce`.

## Integration

- `rustok-cart` calls the canonical root `TaxCalculationPort` factory for recalculating cart tax lines;
- the root in-process factory retains safe local outcome context while the legacy `ports` module path
  remains available for compatibility;
- checkout transfers the provider-aware tax snapshot to `rustok-order`;
- transport surface is still published through `rustok-commerce`.

## Context contracts

- [Tax calculation policy context](./calculation-port-policy-context.md)
- [Tax calculation local outcome context](./calculation-local-context.md)
- [Implementation plan for `rustok-tax`](./implementation-plan.md)

## File map

| File | Responsibility |
| --- | --- |
| `src/lib.rs` | Public Rust facade for Cart and other consumers |
| `src/module.rs` | Native metadata and empty migration source |
| `src/domain/calculation.rs` | Regional policy and checked decimal arithmetic using canonical currency rounding |
| `src/dto/calculation.rs` | Serializable calculation input, taxable amounts and tax lines |
| `src/services/calculation.rs` | Provider selection, default provider and tax-line orchestration |
| `src/ports/calculation.rs` | Read/deadline admission, owner invocation, result validation and safe errors |
| `src/ports/in_process.rs` | Canonical factory and caller-context diagnostics |
| `src/tests/contract.rs` | Native module and public contract assertions |

Cart owns the checkout flow and supplies the current region and channel policy
through `TaxCalculationInput`. To change tax policy or rounding, start in
`src/domain/calculation.rs`. To bind another approved provider, implement
`TaxProvider` and compose it via `TaxService::with_provider` behind the same
port. Verify provider selection, missing-provider errors, currency rounding,
deadline admission and result validation through the in-process port.

## Verification

- `cargo xtask module lint tax --embedded` for the shared source contract;
- targeted unit tests in `rustok-tax`;
- static policy and local-outcome context guards under `scripts/verify`;
- compile-check for `rustok-tax`, `rustok-cart`, `rustok-order`, `rustok-commerce`.
