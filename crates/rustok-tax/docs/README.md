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

- `rustok-cart` calls `TaxService` for recalculating cart tax lines;
- checkout transfers the provider-aware tax snapshot to `rustok-order`;
- transport surface is still published through `rustok-commerce`.

## Verification

- targeted unit tests in `rustok-tax`;
- compile-check for `rustok-tax`, `rustok-cart`, `rustok-order`, `rustok-commerce`.
