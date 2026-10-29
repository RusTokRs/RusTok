# rustok-product-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

## Purpose

For full public interface and contract definitions, see [CRATE_API.md](CRATE_API.md).

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
- Build native storefront services from `HostRuntimeContext` DB and typed
  `TransactionalEventBus` host handles without a package-local framework runtime
  or framework-specific outbox adapter.
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
- Render the catalog facet panel (`ui::leptos::CatalogFacetFilters`) from the owner-resolved facet
  contract: every bucket is a link that flips one `code=value` selection in the URL
  (`core::build_catalog_facet_toggle_query`), unbounded facet domains keep the free-form filter
  input, and the panel owns no client-side state. Facet semantics come from the shared
  `rustok-grid` contract; counting stays Product-owned.

## Entry points

- `ProductView` re-exported from `ui::leptos`
- `core::build_route_input`
- `core::build_selected_product_view_model`
- `core::build_pricing_href`
- `transport::fetch_products`
- `transport::fetch_catalog_facets`
- `ui::leptos::CatalogFacetFilters`

See also `../README.md` and `../docs/README.md`.


## Native Pricing public errors (2026-10-18)

The native Pricing error adapter delegates Foundation/Core/Rich classification and bounded
service diagnostics to `rustok-pricing::map_pricing_service_error`; it renders only static
message/code and a fresh reference. It no longer logs the original pricing error or maintains
a transport-local classifier. See the [Pricing owner contract](../../rustok-pricing/docs/pricing-owner-port-error-safety.md#nested-error-and-public-consumer-continuation-2026-10-18).
Success projections and price contexts are unchanged. Source-only guards and formatting do
not certify compilation, SSR execution, or other catalog context-extractor diagnostics.


## Native storefront context admission (2026-10-18)

Catalog list, facets, detail/Pricing and search options now require the canonical trusted
`RequestContext` before owner reads. Extractor rejection is no longer treated as an absent
optional context; valid anonymous/no-channel contexts remain allowed. Raw rejection/channel/
locale/correlation payload is removed from affected context diagnostics. See the
[current contract](../docs/product-public-error-diagnostic-safety.md#native-storefront-context-admission-continuation-2026-10-18).
This is source-only, unintegrated work: five new negative guard scenarios are unrun;
compilation, host middleware composition and mounted behavior remain maintainer gates.
