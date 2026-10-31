# @rustok/search-frontend API Specification & Contract

## 1. Package Overview & Role
`@rustok/search-frontend` (`apps/next-frontend/packages/search`) is the Next.js storefront client package for RusToK catalog search, autocomplete typeahead suggestions, and faceted filtering. It encapsulates headless GraphQL search operations and provides interactive UI components for instant search experiences with parity to the Leptos storefront search module (`crates/modules/rustok-search/storefront`).

- **Package Name**: `@rustok/search-frontend`
- **Module Directory**: `apps/next-frontend/packages/search`
- **Type**: ESM Package (`"type": "module"`)
- **Key Invariants**:
  - Live search against `storefrontSearch` GraphQL query with faceted bucketing and tookMs metrics.
  - Typeahead autocomplete against `storefrontSearchSuggestions` query with minimum 2-character activation threshold.
  - Surface defaults and filter presets loaded dynamically via `storefrontSearchFilterPresets`.
  - Non-swallowing error propagation across all headless operations.
  - Seamless integration with Next.js App Router through `@rustok/search-frontend` exports.

---

## 2. Exported Symbols & Contracts

### GraphQL Query Constants (`src/api.ts`)
- `STOREFRONT_SEARCH_QUERY`: GraphQL document for live catalog search preview with facet bucketing.
- `STOREFRONT_FILTER_PRESETS_QUERY`: GraphQL document for preset retrieval.
- `STOREFRONT_SUGGESTIONS_QUERY`: GraphQL document for search term and document suggestions.

### Headless Operations & Pure Helpers (`src/api.ts`)
- `parseCsv(value: string): string[]`: Pure utility to split, trim, and filter non-empty comma-separated string segments.
- `fetchStorefrontSearch(query, presetKey, filters, props): Promise<SearchPreviewPayload>`: Executes search query with channel, category, attribute, and sort criteria; propagates GraphQL execution errors.
- `fetchStorefrontFilterPresets(props): Promise<SearchFilterPreset[]>`: Fetches configured search presets; propagates GraphQL errors.
- `fetchStorefrontSuggestions(query, props): Promise<SearchSuggestion[]>`: Fetches autocomplete suggestions (limit 6); propagates GraphQL errors.

### React Components (`src/index.tsx`)
- `SearchStorefrontPage(props: SearchStorefrontPageProps): React.JSX.Element`: Main interactive search experience component with deferred input, transition states, filter presets, catalog filter accordion, suggestion dropdown, and result item cards.

### Data Types (`src/api.ts`)
- `SearchStorefrontPageProps`: Component props defining GraphQL executor, auth token, tenant slug, locale, initial query, and filter options.
- `SearchCatalogFilters`: Form model for channel, category, attribute code, attribute values, min/max ranges, and sort order.
- `SearchCatalogFilterOption`: Label-value option descriptor for datalists and dropdowns.
- `SearchSuggestion`: Autocomplete suggestion record with text, kind, locale, url, and relevance score.
- `SearchResultItem`: Individual search result entity with ID, type, source module, title, snippet, score, locale, url, and raw payload.
- `SearchFacetGroup`: Faceted filter bucket group.
- `SearchPreviewPayload`: Aggregated search preview results envelope.
- `SearchFilterPreset`: Preconfigured filter preset definition.

---

## 3. Architecture & FFA Placement
- **Fluid Frontend Architecture (FFA)**:
  - Framework-agnostic GraphQL client functions and data transformers live in pure TypeScript `src/api.ts` with zero React dependencies.
  - Headless operations can be used independently or in server components.
  - React interactive components live in `src/index.tsx`, consuming `api.ts` functions through React transitions and deferred values.

---

## 4. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - `parseCsv`: handles empty strings, whitespace-only strings, trailing commas, and returns clean string arrays.
   - `fetchStorefrontSearch`: limits search preview results to 12 items (`limit: 12, offset: 0`), trims all string filters (`channelId`, `attributeCode`, `attributeMin`, `attributeMax`, `sortAttributeCode`).
   - `loadSuggestions`: enforces minimum query length (`trimmed.length >= 2`) before dispatching network requests, preventing spam and empty suggestion lookups.
2. **Branch Completeness & Anti-Swallowing**:
   - `fetchStorefrontSearch`, `fetchStorefrontFilterPresets`, and `fetchStorefrontSuggestions` inspect `response.errors` and throw explicit typed `Error` instances containing the backend message rather than swallowing failures.
   - Calling view sets typed error states (`searchError`, `presetsError`, `suggestionsError`) rendered to the user.
3. **State Transition Integrity**:
   - Input clearing immediately clears active results and suggestions without lingering stale data.
   - React transitions (`startResultsTransition`, `startSuggestionsTransition`, `startPresetsTransition`) ensure background queries do not block main thread input responsiveness.
4. **Side-Effect Completeness**:
   - Document suggestion clicks perform direct navigation (`window.location.href = suggestion.url`).
   - Query suggestions update both the input query state and trigger immediate search result loads.
5. **Algorithmic Correctness**:
   - Preset toggle logic toggles active preset off when clicking an already-selected preset (`isSelected ? '' : preset.key`).
   - Facet bucket mapping handles both localized labels and fallback raw bucket values.
6. **Order of Operations & Concurrency**:
   - Query input uses `React.useDeferredValue(query)` to prioritize keystrokes over suggestion fetch updates.
   - Effect callbacks use `React.useEffectEvent` to avoid stale closures while keeping dependencies minimal.

---

## 5. Security & Anti-Defect Guarantees
- **Tenant & Auth Isolation**: All GraphQL operations strictly forward the viewer's `token` and `tenantSlug`, preventing cross-tenant data leaks.
- **Contract Unit Tests**: Verified by `packages/search/src/search-frontend.test.mjs` (5 unit tests covering CSV parsing, search queries, filter presets, suggestions, and GraphQL error anti-swallowing).
