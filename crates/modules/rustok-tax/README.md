# rustok-tax

## Purpose

`rustok-tax` is the default tax-domain foundation for the commerce family.

See [CRATE_API.md](CRATE_API.md) for the verified public crate API specification and platform capability bindings.

## Responsibilities

- Own the tax calculation contract and provider seam instead of leaving tax
  runtime hardcoded inside cart or checkout services.
- Provide a default `region_default` provider that preserves the current
  region-based `tax_rate` / `tax_included` behavior while exposing a stable
  provider boundary for future external tax engines.
- Select the active provider through the current region policy hook
  `regions.tax_provider_id`, rejecting unknown providers instead of
  silently falling back.
- Keep tax-line snapshots provider-aware through a typed `provider_id`
  contract.
- Publish the owner-managed in-process `TaxCalculationPort` factory so cart
  consumers never construct or invoke `TaxService` directly.

## Interactions

- Used by `rustok-cart` as the source of truth for cart tax calculation.
- Snapshot output flows through `rustok-commerce` checkout into `rustok-order`.
- Does not own storefront or admin transport yet; transport remains in
  `rustok-commerce` while the domain seam stabilizes.

## Entry points

- `TaxModule`
- `TaxService`
- `TaxCalculationInput`
- `TaxCalculationResult`

Tax is the embedded reference for a native provider without owner storage or
Page Builder participation. Cart supplies the policy snapshot and invokes the
root `in_process_tax_calculation_port` factory. The provider can run entirely
in-process without database or host setup. See the [file map](./docs/README.md#file-map).

## Verification

- `cargo xtask module lint tax --embedded`
- `cargo test -p rustok-tax --lib`
- `npm run verify:tax:fba`

See also `docs/index.md`.
