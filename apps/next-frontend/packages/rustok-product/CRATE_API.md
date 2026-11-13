# `@rustok/product-frontend` API Specification

## 1. Overview & Ownership

`@rustok/product-frontend` is the Next.js storefront package for the `rustok-product` module (`crates/modules/rustok-product`). It powers customer-facing catalog browsing, faceted filtering, category navigation, multi-attribute product detail pages, and dynamic slot registration on the storefront home page (`home:afterHero`).

- **Package Name**: `@rustok/product-frontend`
- **Location**: `apps/next-frontend/packages/rustok-product`
- **Owner**: `rustok-product` domain module
- **Runtime Environment**: Next.js App Router (React 19, Server Components & Client Components)

---

## 2. Public API Surface & Exports

### Types

- `StorefrontProductListItem`: Product card descriptor with media, pricing range (`priceFrom`), status, and metadata.
- `StorefrontProductDetail`: Full product view model with variants, translations, images, and localized attributes.
- `StorefrontProductVariant`: Variant definition with inventory flags, SKU, and prices.
- `StorefrontProductPricing`: Pricing overlay payload with catalog prices and context-resolved effective prices.
- `ProductCatalogFacet`: Bounded and unbounded faceted attribute counts for drill-down catalog filtering.
- `ProductCatalogSearchOptions`: Available category and attribute filter options.
- `CatalogFacetControls`: Active URL route filter state (`search`, `categoryId`, `sortBy`, `sortDirection`, `attributeFilters`, `currencyCode`).
- `ProductSpecificationRow`: Formatted key-value attribute row for product specification tables.

### Operations & Queries

- `fetchStorefrontProducts(graphql, locale, filter?, tenantSlug?): Promise<StorefrontProductListResponse>`:
  - Fetches paginated storefront catalog products matching search text, category, attributes, and currency. Propagates GraphQL errors.
- `fetchStorefrontProduct(graphql, handle, locale?, tenantSlug?): Promise<StorefrontProductDetail | null>`:
  - Fetches detailed product information by handle. Propagates GraphQL errors.
- `fetchStorefrontProductPricing(graphql, handle, locale?, options?, tenantSlug?, token?): Promise<StorefrontProductPricingDetail | null>`:
  - Resolves and validates a correlated Pricing observation (context, timestamp and variants) for the requested handle.
- `fetchStorefrontCatalogFacets(graphql, locale, filter?, facetCodes, tenantSlug?): Promise<ProductCatalogFacet[]>`:
  - Loads owner-counted attribute bucket counts for faceted navigation.
- `fetchCatalogSearchOptions(requestOrGraphql, locale?, tenantSlug?): Promise<ProductCatalogSearchOptions>`:
  - Queries search options and category taxonomies.

### Pure Domain Helpers

- `mergeProductPricing(variants, pricingVariants): StorefrontProductVariant[]`:
  - Joins pricing data strictly by `variant.id`. Fails closed if pricing is absent; preserves catalog inventory without numeric coercion.
- `buildProductSpecifications(product, labels): ProductSpecificationRow[]`:
  - Builds display-ready specifications table rows, localizing boolean values and discarding empty attributes.
- `buildProductSpecificationLabels(locale?): ProductSpecificationLabels`:
  - Resolves localized specification titles and boolean yes/no labels (`Да`/`Нет` for `ru`, `Yes`/`No` for others).
- `parseAttributeFilters(value?)` / `serializeAttributeFilters(filters)`:
  - Parses and serializes semicolon-joined `code=value` route parameters.
- `toggleAttributeFilter(filters, code, value)` / `clearAttributeFilterCode(filters, code)`:
  - Stable URL filter state manipulators.

### UI Components & Slots

- `CatalogSection`: Featured products section registered as a storefront slot in `home:afterHero`.
- `ProductCard`: Responsive catalog card with primary image, bundle/sale badges, vendor, and price display.
- `ProductGrid`: Catalog grid container with embedded `ProductFilters`, pagination controls, and empty-state placeholders.
- `ProductFilters`: URL-driven filter bar with search input, category dropdown, sort selectors, and drill-down facet panels.
- `ProductDetailView`: Rich product detail view with multi-image gallery, variant switcher, inventory status, price breakdown, quantity stepper, specifications table, and `@rustok/cart-frontend` integration.

---

## 3. Architecture & Data Flow

1. **Slot Registration**:
   - Registers `product-featured-catalog` in `home:afterHero` via `registerStorefrontModule`.
2. **Stateless URL Faceting**:
   - Facet selection round-trips exclusively through URL query strings (`attribute_filters`).
   - Deep links remain shareable, bookmarkable, and idempotent.
3. **Decoupled Pricing Overlays**:
   - Product catalog data owns identity, images, and inventory.
   - Pricing overlays from the pricing engine join strictly by `variant.id`.
   - Never falls back to stale out-of-context prices when explicit pricing rules are active.

---

## 4. Universal Logic Dimensions

### 1. Preconditions & Bounds
- Input handles, categories, and attribute codes are trimmed; empty or whitespace-only inputs return safe default envelopes.
- Pagination bounds: `page` clamped to `>= 1`, `perPage` defaults to 12.
- Total pages calculated safely: `Math.max(1, Math.ceil(total / perPage))`.
- The buyer accepts only whole quantities from 1 through 2,147,483,647 and checks the selected SKU's stock policy; invalid input is shown as invalid, not clamped into a different purchase.

### 2. Branch Completeness & Anti-Swallowing
- `fetchStorefrontProducts`, `fetchStorefrontProduct`, `fetchStorefrontCatalogFacets`, and `fetchCatalogSearchOptions` inspect `response.errors?.length` and propagate typed domain errors instead of masking failures with empty data envelopes.

### 3. State Transition Integrity
- Variant switching in `ProductDetailView` re-evaluates active price, compare-at amounts, inventory availability, and image gallery.
- The Product buyer consults the Cart provider's `enabled`, loading, mutation and uncertainty state before enabling or starting an add. Capability changes invalidate its quote scope; Cart independently fences asynchronous writes.

### 4. Side-Effect Completeness
- An explicitly confirmed, successful Cart add opens the drawer; an unconfirmed write is not reported as success. The landing handoff only navigates to Product and performs no Cart mutation.
- Search filter updates leverage React 19 `useTransition` for non-blocking concurrent URL transitions.

### 5. Calculation & Algorithmic Correctness
- The Product buyer displays the owner's decimal price strings without JavaScript money arithmetic. Cart and checkout have separate presentation limitations documented by their owner.
- Boolean attributes are localized via vocabulary mapping (`true` -> `Yes`/`Да`, `false` -> `No`/`Нет`).
- Attribute filter toggling preserves stable element order across repeated modifications.

### 6. Order of Operations & Concurrency
- `Promise.all` batches parallel queries for products and search options.
- Facet counting queries operate across the unpaginated catalog filter set for accurate counts.

---

## 5. Verification & Testing

- **Contract Unit Tests**: `apps/next-frontend/packages/rustok-product/src/catalog/pricing.test.mjs` and `src/product-frontend.test.mjs`.
- **Execution Command**:
  ```bash
  npm --prefix apps/next-frontend run test:product
  ```
- **Buyer and Cart unit contracts**: `npm --prefix apps/next-frontend run test:storefront-buyer`.
- **Targeted React/browser harness**: `npm --prefix apps/next-frontend run test:e2e:fly` exercises the Product buyer, Cart provider and landing handoff with fake router, translations and GraphQL; it is not a live host or checkout test.
- **Type Checking**:
  ```bash
  npm --prefix apps/next-frontend run typecheck
  ```

