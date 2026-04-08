# rustok-pricing-storefront

## Purpose

`rustok-pricing-storefront` provides the module-owned Leptos storefront route for
public pricing discovery.

## Responsibilities

- Render the public pricing atlas for published catalog entries.
- Read pricing summary and variant-level price data through native `#[server]`
  functions backed by `rustok-pricing::PricingService`.
- Keep the existing GraphQL storefront contract as a parallel fallback path.
- Consume the host-provided effective locale from `UiRouteContext`.

## Entry points

- `PricingView`
- `api::fetch_storefront_pricing`

See also `../README.md` and `../docs/README.md`.
