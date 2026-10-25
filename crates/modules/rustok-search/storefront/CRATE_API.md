# rustok-search-storefront / CRATE_API

## Public surface

### Components
- `SearchView`: Canonical Leptos storefront search results view and autocomplete query interface.
- `SearchCatalogFilterOption`: Catalog filter option descriptor for storefront navigation.

### Public modules
- `core`: Pure Rust framework-agnostic search route query parser (`parse_search_route_filters`), view models (`SearchResultsViewModel`, `SearchResultItemViewModel`, `SearchFacetViewModel`), facet bucket formatters, and query builders.
- `model`: Storefront search transfer DTOs (`SearchPreviewPayload`, `SearchFacetGroup`, `SearchFilterPreset`, `SearchSuggestion`).
- `transport`: Dual-path storefront transport facade (`fetch_search`, `fetch_suggestions`, `fetch_filter_presets`, `fetch_forum_search_with_filters`, `SearchTransportError`).

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for the `rustok-search` module.
- Dual-path transport via `UiTransportPath`:
  - `NativeServer`: Monolith SSR / Hydration server functions powered by `leptos_axum`.
  - `Graphql`: Headless client transport powered by `rustok-graphql`.
- Full localization: Fluent bundles (`locales/en.ftl`, `locales/ru.ftl`) integrated via `rustok-ui-i18n`.

## Domain Collaborations
- Integrates with backend `rustok-search` for full-text search, facet counts, and instant autocomplete suggestions.
- Storefront channel authority: search queries respect channel-specific visibility, automatically excluding documents outside active storefront channel boundaries.

## Minimum Contract Set

### Input DTOs/Commands
- `SearchRouteFilterQuery`: URL query string parameters for entity types, source modules, category IDs, statuses, attribute filters, and sort options.
- `SearchPreviewFilters`: structured filter options dispatched to transport layer.
- `SearchTransportError`: error enum representing server function or GraphQL failures.

### Domain Invariants
- Symmetric FFA boundary: `core/` contains 0 UI framework imports (`leptos::*`, `dioxus::*`); all URL query decoding, facet formatting, and view model assembly are pure Rust functions.
- UI components in `ui/leptos` contain zero business rules, domain validations, or direct database access.
- Safe channel boundaries: Public storefront searches fail closed if channel or tenant context is missing or mismatched.

### Events / Outbox Side Effects
- Emits client search click events for search relevance analytics and CTR metrics.

### Errors / Failure Codes
- `SearchTransportError::ServerFn`: Monolith server function failure or serialization error.
- `SearchTransportError::Graphql`: Headless GraphQL execution or network failure.
- `SearchTransportError::Validation`: Malformed query parameter or invalid filter argument.
