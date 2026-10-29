# rustok-product-storefront / CRATE_API

## Public surface

### Components
- `ProductView`: root storefront component rendering product details, variants, media gallery, and purchase actions.
- `CatalogFacetFilters`: dynamic facet filtering panel for catalog browsing.

### Data Models & Queries
- `ProductCatalogFacet`, `ProductCatalogFacetValue`: facet descriptor models for category/attribute filtering.
- `ProductCatalogSearchOptions`, `ProductCatalogSearchOption`: search and filter parameter containers.
- `fetch_catalog_facets`: transport query helper retrieving facet distribution for active query filters.
- `fetch_catalog_search_options`: transport query helper resolving search parameters and metadata.

## Platform Capability Bindings
- **Storefront Discovery Route**: Mounts customer-facing catalog rail and product detail views in `apps/storefront`.
- **Facet Navigation**: URL-driven facet toggle queries (`code=value`) integrated with `rustok-grid` facet semantics.
- **Dual Transport Integration**: Operates over native Leptos `#[server]` functions backed by `CatalogService` with parallel GraphQL fallback.

## Domain Collaborations
- **`rustok-product`**: Core module providing published product data, options, variant combinations, and inventory state.
- **`rustok-pricing`**: Resolves customer-facing pricing tiers and deep-links directly to pricing inspection.
- **`rustok-commerce-foundation`**: Cart additions and order initialization handoff.

## Minimum Contract Set

### Input DTOs/Commands
- `ProductCatalogSearchOptions`: query parameters for faceted catalog searches.
- `ProductCatalogFacet`: facet domain descriptor with bucket counts and selection flags.

### Domain Invariants
- **Published Only**: Storefront surfaces strictly expose products with `published` status; draft or archived products are inaccessible.
- **Seller Identity Boundaries**: `seller_id` represents the authoritative seller boundary; `vendor` serves purely as display/merchandising copy.
- **Locale Parity**: Storefront product copy resolves against the host's effective locale before falling back to canonical translations.

### Events / Outbox Side Effects
- Emits analytics and product-view telemetry events.
- Dispatches cart item additions without mutating catalog persistence.

### Errors / Failure Codes
- `ProductTransportError`: typed transport error retaining failure evidence across native and GraphQL paths.
- DOM error evidence attributes (`data-product-transport-*`) for headless observability.
