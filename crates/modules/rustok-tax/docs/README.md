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

## Verification

- targeted unit tests in `rustok-tax`;
- static policy and local-outcome context guards under `scripts/verify`;
- compile-check for `rustok-tax`, `rustok-cart`, `rustok-order`, `rustok-commerce`.
