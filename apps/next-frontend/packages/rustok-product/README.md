# @rustok/product-frontend

Next.js storefront client package for the `rustok-product` module (`crates/modules/rustok-product`).

## Responsibilities

- Renders storefront product catalog grids, featured product sections, and rich product detail views.
- Registers the `product-featured-catalog` slot in `home:afterHero`.
- Pure URL-driven facet filtering (`ProductFilters`) with drill-down counting, multi-select toggling, and query serialization (`attribute_filters`).
- Product specifications presentation (`buildProductSpecifications`) with localized boolean labels (`Yes`/`No`, `Да`/`Нет`) and display-ready attributes.
- Authoritative pricing overlays (`mergeProductPricing`) joining strictly by variant ID without numeric coercion or stale fallback.
- Typed GraphQL data fetching operations (`STOREFRONT_PRODUCTS_QUERY`, `STOREFRONT_PRODUCT_QUERY`, `STOREFRONT_PRICING_PRODUCT_QUERY`, `STOREFRONT_CATALOG_FACETS_QUERY`, `STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY`).

## Architecture & Integration

`@rustok/product-frontend` implements Fluid Frontend Architecture (FFA) principles:
- Framework-agnostic view-model helpers in `catalog/pricing.ts`, `catalog/facets.ts`, and `catalog/specifications.ts`.
- Direct integration with `@rustok/cart-frontend` for "Add to Cart" state management with quantity steppers and inventory limits.
- SEO-ready metadata generation and responsive media gallery with aspect-ratio containment.

## Verification

Run scoped tests:
```bash
npm --prefix apps/next-frontend run test:product
```

Run host type check:
```bash
npm run typecheck:next-frontend
```

