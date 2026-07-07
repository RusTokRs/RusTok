# rustok-product-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

## Purpose

`rustok-product-storefront` provides the module-owned Leptos storefront route for
published catalog discovery.

## Responsibilities

- Render the public catalog rail and selected product detail for the current
  tenant.
- Keep storefront route/query state normalization, selected-product view-model
  composition, pricing/seller formatting, pricing-context sanitization/defaulting,
  and pricing deep-link construction in framework-agnostic `src/core.rs`, so
  Leptos remains a thin host-context/render adapter before calling transport.
- Read storefront product data through `src/transport/`, which keeps native
  `#[server]` functions backed by `rustok-product::CatalogService` as the
  preferred path.
- Keep the existing GraphQL storefront contract as a parallel fallback adapter in
  `src/transport/graphql_adapter.rs`.
- Preserve host-visible native/GraphQL selected-path failure evidence through
  `ProductTransportError`, core `ProductTransportErrorDomEvidence`, and the
  Leptos error adapter data attributes (`data-product-transport-*`).
- Treat `storefrontProduct -> variants.prices` as a catalog compatibility
  snapshot and show resolved price data through a separate pricing-module hook
  backed by `rustok-pricing` in native server functions and the parallel GraphQL path.
- Surfaces `seller_id` as the storefront seller boundary while keeping `vendor`
  as a merchandising/display label only.
- Links directly into `rustok-pricing/storefront` with the current handle and
  pricing context so catalog browsing can pivot into pricing inspection without
  rebuilding the query state by hand.
- Consume the host-provided effective locale from `UiRouteContext` and resolve selected product copy against that locale before falling back to another translation.

## Entry points

- `ProductView` re-exported from `ui::leptos`
- `core::build_route_input`
- `core::build_selected_product_view_model`
- `core::build_pricing_href`
- `transport::fetch_products`

See also `../README.md` and `../docs/README.md`.
