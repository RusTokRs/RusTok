# rustok-product-admin / CRATE_API

## Public surface

### Entry Point & Pages
- `ProductAdmin`: mounted root admin component rendered into host admin applications.
- `ProductGridPage`: product catalog grid listing, search, and bulk operations page.
- `ProductEditorPage`: detailed product authoring and variant management editor page.
- `AttributesPage`: typed attribute schema and category binding page.
- `CategoriesPage`: category hierarchy management page.

### Core View Models & Helpers
- `ProductKind`: enumerated product classification (`Physical`, `Digital`, `Service`).
- `product_grid_columns`: standard column definitions for the product catalog grid.
- `admin_catalog_facets_to_grid`: facet view transformer for grid filter panels.
- `fetch_catalog_search_options`: transport query helper retrieving search and filter options.

## Platform Capability Bindings
- **Mounted Admin Surface**: Registers `ProductAdmin` as the canonical admin entrypoint via manifest-driven code generation.
- **Variant Axes Configuration**: Exposes dynamic variant axes management (`SetVariantAxesDraft`, `VariantAxisConfig`, `VariantAxisDraft`).
- **Facet Panels & Filtering**: Exposes facet aggregation models (`ProductAdminFacetPanelView`, `AdminCatalogFacet`).

## Domain Collaborations
- **`rustok-product`**: Core domain module providing product entities, variants, and validation logic.
- **`rustok-pricing`**: Deep-links directly to pricing admin panels and renders authoritative pricing previews.
- **`rustok-seo-panel`**: Integrates product-level SEO metadata editing within the product form.
- **`rustok-grid-leptos`**: Standard data grid component rendering the product catalog with sorting and pagination.

## Minimum Contract Set

### Input DTOs/Commands
- `SetVariantAxesDraft`: DTO defining the complete axis draft for multi-variant products.
- `VariantAxisDraft`: draft representation of a single variant axis.
- `VariantAxisValueDraft`: draft value option within a variant axis.
- `ProductCatalogSearchOptions`: query parameters for product searching and filtering.

### Domain Invariants
- **Non-Swallowed Errors**: Catalog mutations (status changes, deletions, variant updates) must inspect and handle operation results instead of silently ignoring errors.
- **Pricing Authority**: Catalog variant prices are compatibility snapshots; authoritative pricing controls and validations belong to `rustok-pricing`.
- **Locale Parity**: Admin interface strings use package-owned Fluent translations (`en.ftl`, `ru.ftl`) synchronized with host route context.

### Events / Outbox Side Effects
- Triggers product lifecycle mutations (publish, archive, draft) producing outbox domain events.
- Dispatches category hierarchy and variant axis modifications through native server or GraphQL adapters.

### Errors / Failure Codes
- `UiError`: form validation and presentation error states.
- `CatalogSearchOptionsErrorContext`: diagnostic error context for catalog search failures.
