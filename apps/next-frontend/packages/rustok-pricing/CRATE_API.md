# `@rustok/pricing-frontend` API Specification

## 1. Overview & Ownership

`@rustok/pricing-frontend` is the Next.js storefront client package for the `rustok-pricing` module (`crates/modules/rustok-pricing`). It displays product pricing, effective promotional discounts, currency conversions, volume tiered prices, and sales channel price lists across customer-facing storefront routes.

- **Package Name**: `@rustok/pricing-frontend`
- **Location**: `apps/next-frontend/packages/rustok-pricing`
- **Owner**: `rustok-pricing` domain module
- **Runtime Environment**: Next.js App Router (React 19, Client Components)

---

## 2. Public API Surface & Exports

### Types

- `PricingPriceListOption`: Available price list descriptor (`id`, `name`, `listType`, `channelId`, `ruleKind`, `adjustmentPercent`).
- `PricingChannelOption`: Available sales channel descriptor (`id`, `slug`, `name`, `isActive`, `isDefault`, `status`).
- `PricingPrice`: Basic price point (`currencyCode`, `amount`, `compareAtAmount`, `discountPercent`, `onSale`).
- `PricingEffectivePrice`: Context-resolved effective price point including rule origins, channel/region scoping, and tier minimum/maximum quantities.
- `PricingVariant`: Product variant with catalog prices and optional effective price.
- `PricingProductDetail`: Detailed product view model with localized translations and variant pricing matrices.
- `PricingProductListItem`: Product card summary with pricing metrics, currency support, and variant counts.
- `PricingProductList`: Paginated container for product listings.
- `PricingResolutionContext`: Active pricing query context (`currencyCode`, `regionId`, `priceListId`, `channelId`, `quantity`).
- `StorefrontPricingData`: Aggregated storefront pricing payload containing products, selected product, channels, and price lists.

### Operations & Queries

- `STOREFRONT_PRODUCTS_QUERY`: GraphQL document fetching products, channels, and active price lists for storefront display.
- `STOREFRONT_PRODUCT_QUERY`: GraphQL document resolving effective price for a single product handle under specific context parameters.
- `fetchStorefrontPricing(graphql, query, token?, tenantSlug?): Promise<StorefrontPricingData>`:
  - Fetches product list and optionally queries single product pricing when `query.selectedHandle` is provided.
  - Propagates backend GraphQL errors if queries encounter execution failures.

### UI Components

- `PricingContextBar`: Interactive control bar allowing customers to switch sales channels, active price lists, target currency, and purchase quantity.
- `PricingProductCard`: Product card showing product type, title, vendor, handle, sale badge, variant count, and selection toggle.
- `PricingProductDetailView`: Product detail panel rendering variant price matrix, comparing base prices with resolved effective prices and discount tags.
- `PricingView`: Full-page view coordinating context state, asynchronous data fetching, loading skeleton pulses, error banner alerts, and selection dismissal.

---

## 3. Architecture & Data Flow

1. **Context Resolution**:
   - The user selects or defaults context parameters: currency code (e.g. `USD`, `EUR`, `RUB`, `GBP`), sales channel, price list, and quantity.
2. **Dual-Query Aggregation**:
   - `fetchStorefrontPricing` simultaneously executes `STOREFRONT_PRODUCTS_QUERY` and, if a product handle is selected, `STOREFRONT_PRODUCT_QUERY`.
   - Propagates server errors without swallowing.
3. **Effective Price Matrix**:
   - The product detail view matches base catalog prices against resolved effective prices by currency code.
   - Highlights on-sale savings with strikethrough compare-at amounts and `-X%` pill badges.

---

## 4. Universal Logic Dimensions

### 1. Preconditions & Bounds
- Input quantity in `PricingContextBar` is sanitized and clamped: `Math.max(1, parseInt(e.target.value, 10) || 1)`, bounded by HTML `min="1"` and `max="999"`.
- Defaults to `currencyCode = 'USD'` and `quantity = 1` when resolution context is uninitialized.
- Null or undefined channels and price lists safely fallback to empty arrays (`[]`).

### 2. Branch Completeness & Anti-Swallowing
- `fetchStorefrontPricing` checks `productsRes.errors?.length` and `productRes.errors?.length`, throwing descriptive errors that trigger the error state in `PricingView`.
- Errors are displayed via accessible `<div role="alert">` banners.

### 3. State Transition Integrity
- Selection toggle in `PricingProductCard` smoothly handles single-selection and deselects if the same handle is clicked again.
- Clearing selection via `✕ Clear selection` resets `selectedHandle` to `null`.

### 4. Side-Effect Completeness
- `useEffect` in `PricingView` tracks cancellation via `let cancelled = false; return () => { cancelled = true; };` preventing unmounted component state updates and race conditions during fast context changes.

### 5. Calculation & Algorithmic Correctness
- `basePrice` fallback logic finds matching currency first (`currencyCode === context.currencyCode`), falling back to `variant.prices[0]` or `'—'` when unpriced.
- Sale status evaluates both `effective.onSale` and `basePrice.onSale`.

### 6. Order of Operations & Concurrency
- `Promise.all` executes independent queries in parallel to minimize latency.
- Fast successive changes to currency or quantity cleanly abort obsolete asynchronous resolution batches.

---

## 5. Verification & Testing

- **Contract Unit Tests**: `apps/next-frontend/packages/rustok-pricing/src/pricing-frontend.test.mjs`.
- **Execution Command**:
  ```bash
  npm --prefix apps/next-frontend run test:pricing
  ```
- **Type Checking**:
  ```bash
  npm run typecheck:next-frontend
  ```
