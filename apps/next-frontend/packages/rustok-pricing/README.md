# @rustok/pricing-frontend

Storefront pricing package for RusTok Next.js storefront (`apps/next-frontend`).

## Responsibilities

- Provides public pricing visibility, currency selector, quantity-based pricing matrix, and promotional sale badges.
- Exposes typed GraphQL operations (`STOREFRONT_PRODUCTS_QUERY`, `STOREFRONT_PRODUCT_QUERY`) and executor `fetchStorefrontPricing`.
- Renders responsive storefront components:
  - `PricingContextBar`: Channel selector, price list switcher, currency selector (USD, EUR, RUB, GBP), and clamped quantity selector.
  - `PricingProductCard`: Product listing item with sale badge, variant count, and interactive selection toggle.
  - `PricingProductDetailView`: Comprehensive product pricing detail view displaying base price vs effective price, compare-at amounts, discount percentage badges, and variant price matrix.
  - `PricingView`: Host page coordinator managing selection state, pricing resolution context, loading skeletons, and error alerts.

## Architecture & Integration

`@rustok/pricing-frontend` is part of the Fluid Frontend Architecture (FFA). It delegates pricing resolution and tax/discount calculations to the backend `rustok-pricing` module while rendering client components with React 19.

## Verification

Run scoped tests:
```bash
npm --prefix apps/next-frontend run test:pricing
```

Run host type check:
```bash
npm run typecheck:next-frontend
```

