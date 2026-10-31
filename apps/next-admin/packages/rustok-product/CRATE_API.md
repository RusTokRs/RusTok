# @rustok/product-admin — Canonical API Specification

## 1. Package Overview

`@rustok/product-admin` provides the module-owned Next.js administration interface for the RusToK Product Catalog platform. It delivers full catalog management including products, multi-variant matrices, categories, dynamic attributes, bundles, pricing policies, and crash-resilient write attempts over the unified `rustok-product` async-graphql backend schema.

- **Package Name**: `@rustok/product-admin`
- **Location**: `apps/next-admin/packages/rustok-product`
- **Owner**: `rustok-product` module frontend surface
- **Status**: Stable / Verified

## 2. Public API Surface

### Navigation

- `productNavItems: NavItem[]`: Declares `/dashboard/product` routes (Products, Categories, Attributes, Bundles) under the `modulePlugins` group with `['p', 'l']` shortcut and `{ role: 'manager' }` access.

### Pages & View Components

- `ProductEditorPage`: Complete product authoring workspace including general info, translations, categories, variants, variant axes, media gallery, product relations, bundles, and SEO settings.
- `CategoriesPage`: Hierarchical category management table with creation dialog.
- `AttributesPage`: Dynamic attribute schemas table, options editor dialog, and schema authoring card.
- `BundlesPage`: Product bundle definitions table and bundle creation dialog.
- `ProductTable`: Virtualized data table with status badges, price presentation, and row actions.

### Core Utilities & Money Arithmetic

- `parsePriceAmount(input: string): string`: Validates that a price is a non-negative decimal amount within 96-bit coefficient bounds (matching `rust_decimal`), rejecting exponential notation or invalid formats.
- `salePercent(amount: string, compareAt?: string | null): string | null`: Computes exact integer discount percentage.
- `discountedPrice(amount: string, percent: number): string`: Calculates discounted price amount using 64-digit decimal precision.
- `normalizeProductStatus(status?: string | null)`: Normalizes status strings to `'DRAFT' | 'ACTIVE' | 'ARCHIVED'`.

### Write Journal & Recovery Locks

- `executeProductWrite(client, attempt)`: Executes idempotent write attempt with localStorage recovery journal and lock serialization.
- `isProductEditorIntent(intent)`: Validates that a browser intent query key corresponds to a valid product editor intent.

### GraphQL API Operations

- `PRODUCTS_QUERY`: Lists paginated products with filter and locale support.
- `PRODUCT_QUERY` / `PRODUCT_DETAIL_QUERY`: Fetches complete product definition including variants, translations, axes, and media.
- `PRODUCT_ATTRIBUTES_QUERY`: Queries filterable and sortable catalog search attributes.
- `CATALOG_CATEGORIES_QUERY`: Queries category taxonomy for search and assignment.
- `CREATE_PRODUCT_MUTATION`: Creates a product with idempotency key.
- `UPDATE_PRODUCT_MUTATION`: Updates a product with revision check and idempotency key.
- `DELETE_PRODUCT_MUTATION`: Deletes a product by UUID.

## 3. Dependencies & Architecture Boundaries

- **Direct Dependencies**: `react: ^19.2.0`, `decimal.js-light: ^2.5.1`, `next: ^15.0.0` (peer / host).
- **Host Integration**: Registered in the Next.js admin registry via `registerAdminModule({ id: 'product', name: 'Product Catalog', navItems: productNavItems })`.
- **Backend Boundary**: Communicates with the `rustok-product` async-graphql schema via `graphqlRequest`.

## 4. Invariants & Security

- **Decimal Money Precision**: Prices are strictly represented as exact decimal strings in major units (e.g. `"19.99"`). No floating-point math, exponent notation, or minor-unit integer conversion is performed in the catalog editor.
- **Fail-Closed Write Safety**: Write attempts serialize browser tab operations using locks, track pending idempotency keys in persistent journals, and reject unauthenticated calls before performing IO.
- **Revision Protection**: Status transitions and product saves check acknowledged revision numbers to prevent stale state overwrites.
- **Tenant Context Enforcement**: All catalog operations enforce active `tenantId`, `tenantSlug`, and `token` parameters.

## 5. Verification & Testing

- **Contract & Unit Tests**: 8 test suites containing 86 unit and contract tests executed via `npm --prefix apps/next-admin run test:product`.
- **Verified Coverage**:
  - `money.test.mjs`: Exact decimal parsing, 96-bit bounds, discount percentage math.
  - `product-create.test.mjs`: Translation preservation, explicit zero prices, payload defaults.
  - `product-editor.test.mjs`: Lifecycle actions, revision advancement, status handler guards.
  - `product-prices.test.mjs`: Variant prices round-tripping, major-unit serialization.
  - `graphql-outcome.test.mjs`: Apollo combined error parsing, typed error preservation.
  - `write-journal.test.mjs`: Persistence snapshots, scrub bodies, lock serialization, tombstone settling.
  - `write-attempt.test.mjs`: Recovery dispatch, caller key forwarding, unknown retry scoping.
  - `product-save-retry.test.mjs`: Save retries, optimistic UI updates, lock recovery.
