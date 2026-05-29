# rustok-product-storefront

## Purpose

`rustok-product-storefront` provides the module-owned Leptos storefront route for
published catalog discovery.

## Responsibilities

- Render the public catalog rail and selected product detail for the current
  tenant.
- Keep storefront route/query state normalization plus pricing/seller view-model
  formatting in framework-agnostic `src/core.rs`, so Leptos remains a thin
  host-context/render adapter before calling transport.
- Read storefront product data through native `#[server]` functions backed by
  `rustok-product::CatalogService`.
- Keep the existing GraphQL storefront contract as a parallel fallback path.
- Treat `storefrontProduct -> variants.prices` as a catalog compatibility
  snapshot and show resolved price data through a separate pricing-module hook
  backed by `rustok-pricing` in native server functions and GraphQL fallback.
- Surfaces `seller_id` as the storefront seller boundary while keeping `vendor`
  as a merchandising/display label only.
- Links directly into `rustok-pricing/storefront` with the current handle and
  pricing context so catalog browsing can pivot into pricing inspection without
  rebuilding the query state by hand.
- Consume the host-provided effective locale from `UiRouteContext` and resolve selected product copy against that locale before falling back to another translation.

## Entry points

- `ProductView`
- `core::build_storefront_route_input`
- `core::build_storefront_pricing_href`
- `api::fetch_storefront_products`

See also `../README.md` and `../docs/README.md`.
