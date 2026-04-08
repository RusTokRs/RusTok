# rustok-product-storefront

## Purpose

`rustok-product-storefront` provides the module-owned Leptos storefront route for
published catalog discovery.

## Responsibilities

- Render the public catalog rail and selected product detail for the current
  tenant.
- Read storefront product data through native `#[server]` functions backed by
  `rustok-product::CatalogService`.
- Keep the existing GraphQL storefront contract as a parallel fallback path.
- Consume the host-provided effective locale from `UiRouteContext`.

## Entry points

- `ProductView`
- `api::fetch_storefront_products`

See also `../README.md` and `../docs/README.md`.
