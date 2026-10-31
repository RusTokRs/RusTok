# @rustok/search-admin — Canonical API Specification

## 1. Package Overview

`@rustok/search-admin` provides the module-owned Next.js administration interface for the RusToK Search platform. It manages search engine selection (PostgreSQL Full-Text Search, Meilisearch, Elasticsearch), index diagnostics, real-time playground testing, search analytics with zero-result monitoring, and dictionary management (synonyms, stopwords, query pinning rules).

- **Package Name**: `@rustok/search-admin`
- **Location**: `apps/next-admin/packages/search`
- **Owner**: `rustok-search` module frontend surface
- **Status**: Stable / Verified

## 2. Public API Surface

### Exported Components & Pages

- `SearchAdminPage` (default export & named): Full client component hosting the 4 operational tabs:
  - `Overview`: Engine statuses, index lag, consistency diagnostics, rebuild triggers.
  - `Playground`: Live search testing with ranking profiles, facets, and scoring inspection.
  - `Analytics`: Query volumes, CTR metrics, zero-result investigations, and search intelligence.
  - `Dictionaries`: Synonyms, stopwords, and pinned query rules.

### Navigation

- `searchNavItems: NavItem[]`: Declares `/dashboard/search` under the `modulePlugins` navigation group.

### API & Data Fetching Queries & Mutations

- `SEARCH_ADMIN_BOOTSTRAP_QUERY`: Loads available engines, active settings, and diagnostics.
- `SEARCH_PREVIEW_QUERY`: Queries search results preview with snippets, facets, and execution time.
- `SEARCH_FILTER_PRESETS_QUERY`: Fetches configured filter presets.
- `SEARCH_LAGGING_DOCUMENTS_QUERY`: Queries documents with indexing lag exceeding thresholds.
- `SEARCH_CONSISTENCY_ISSUES_QUERY`: Queries documents with index/store discrepancies.
- `SEARCH_ANALYTICS_QUERY`: Retrieves search analytics and intelligence candidates.
- `SEARCH_DICTIONARY_SNAPSHOT_QUERY`: Loads synonyms, stopwords, and query rules.
- `TRIGGER_SEARCH_REBUILD_MUTATION`: Triggers full or targeted document reindexing.
- `TRACK_SEARCH_CLICK_MUTATION`: Records search query click telemetry.
- `UPDATE_SEARCH_SETTINGS_MUTATION`: Persists active engine and ranking profiles.
- `UPSERT_SEARCH_SYNONYM_MUTATION`: Adds or updates a synonym mapping.
- `DELETE_SEARCH_SYNONYM_MUTATION`: Removes a synonym mapping.
- `ADD_SEARCH_STOP_WORD_MUTATION`: Registers a stopword.
- `DELETE_SEARCH_STOP_WORD_MUTATION`: Removes a stopword.
- `UPSERT_SEARCH_PIN_RULE_MUTATION`: Pins a document to a specific search query.
- `DELETE_SEARCH_QUERY_RULE_MUTATION`: Removes a query pinning rule.

### Utility Functions

- `parseCsv(value)`: Splits comma-separated values into trimmed non-empty arrays.
- `optionalText(value)`: Trims strings and returns undefined for empty inputs.
- `prettyJsonString(value)`: Safely formats JSON with indentation.
- `parseJsonValue(value)`: Safely parses JSON strings, returning null on syntax errors.
- `extractRankingProfileValue(configText, surface)`: Reads configured ranking profile per surface.
- `extractSurfacePresetsJson(configText, surface)`: Reads filter presets JSON per surface.
- `parseJsonArrayForEditor(label, value)`: Validates that a string parses into a JSON array.
- `mergeRelevanceEditorConfig(input)`: Merges ranking profiles and presets into a validated JSON config.

## 3. Dependencies & Architecture Boundaries

- **Direct Dependencies**: `react: ^19.2.0`, `next: ^15.0.0` (peer / host).
- **Host Integration**: Registered in the Next.js admin registry via `registerAdminModule({ id: 'search', name: 'Search', navItems: searchNavItems })`.
- **Backend Boundary**: Communicates with the `rustok-search` async-graphql schema via `graphqlRequest`.

## 4. Invariants & Security

- **Safe JSON Config Parsing**: Config parser utilities never throw unhandled exceptions on invalid JSON; `parseJsonValue` fails safely to `null`, and `mergeRelevanceEditorConfig` provides descriptive validation errors.
- **Surface Isolation**: Relevance ranking profiles and filter presets are strictly scoped per surface (`search_preview`, `storefront_search`, `admin_global_search`).
- **Telemetry Anti-Swallowing**: Search clicks and query execution telemetry capture error states without breaking end-user search rendering.
- **Tenant Context Enforcement**: All queries and mutations propagate `tenantSlug` and authentication tokens to the backend gateway.

## 5. Verification & Testing

- **Contract Tests**: `packages/search/src/search-admin.test.mjs` executed via `npm --prefix apps/next-admin run test:search`.
- **Verified Coverage**:
  - `searchNavItems` routing and group configuration.
  - GraphQL queries and mutations schema structure.
  - `parseCsv` and `optionalText` boundary conditions.
  - `prettyJsonString` and `parseJsonValue` error resilience.
  - `extractRankingProfileValue` and fallback resolution.
  - `mergeRelevanceEditorConfig` JSON structure validation.
