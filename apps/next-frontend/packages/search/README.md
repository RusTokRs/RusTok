# @rustok/search-frontend

Next.js storefront search package for RusToK.

## Capabilities
- **Storefront Live Search**: Executes faceted catalog queries via `fetchStorefrontSearch` against the GraphQL `storefrontSearch` operation.
- **Typeahead Suggestions**: Real-time suggestion autocomplete via `fetchStorefrontSuggestions` with document link routing.
- **Filter Presets**: Dynamic surface presets (`fetchStorefrontFilterPresets`) for fast search slice filtering.
- **Catalog Facet Filtering**: Support for multi-category IDs, dynamic attribute filters, ranges (min/max), channel IDs, and sort direction.
- **Anti-Swallowing Error Propagation**: Guaranteed typed error propagation from GraphQL response errors across all query executors.

