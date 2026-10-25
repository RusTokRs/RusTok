# rustok-search-admin / CRATE_API

## Public surface

### Components
- `SearchAdmin`: Root Leptos admin component providing the comprehensive search administration workstation (engine configuration, ranking profiles, filter presets, dictionaries, preview playground, diagnostics, analytics, and rebuild controls).
- `SearchCatalogFilterOption`: Catalog filter option descriptor for multi-faceted search administration.

### Public modules
- `core`: Pure Rust framework-agnostic view models (`SearchPreviewViewModel`, `SearchPreviewItemViewModel`, `SearchAnalyticsViewModel`, `SearchDiagnosticsViewModel`, `SearchDictionaryViewModel`), grid column definitions (`query_rules_columns`, `synonyms_columns`), filter matchers, and label formatters.
- `model`: Transfer DTOs (`SearchEngineDescriptor`, `SearchSettingsPayload`, `SearchFacetGroup`, `SearchPreviewPayload`, `SearchDictionarySnapshotPayload`, `SearchAnalyticsPayload`, `SearchDiagnosticsPayload`).

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) admin package for the `rustok-search` module.
- Dual-path transport via `UiTransportPath`:
  - `NativeServer`: Monolith SSR / Hydration server functions powered by `leptos_axum`.
  - `Graphql`: Headless client transport powered by `rustok-graphql`.
- UI tokens and components: Uses `rustok-grid` (`rustok-grid-leptos`) for data grids, paginated tables, column sorting, and filtering.
- Full localization: Fluent bundles (`locales/en.ftl`, `locales/ru.ftl`) integrated via `rustok-ui-i18n`.

## Domain Collaborations
- Integrates with backend `rustok-search` engine registry (`PgSearchEngine` and external connectors).
- Enforces platform RBAC permissions: `SEARCH_SETTINGS_READ` and `SEARCH_SETTINGS_MANAGE`.
- Tenant scope enforcement: `ensure_search_admin_tenant_scope` guarantees tenant equality before permission admission.

## Minimum Contract Set

### Input DTOs/Commands
- `SearchPreviewFilters`: search filter overrides (module exclusions, channel, date bounds, author, solved status).
- `SearchSettingsPayload`: active engine, fallback engine, and JSON engine configuration.
- `SearchDictionaryMutationPayload`: addition, deletion, and editing of synonyms, stop words, and pinned query rules.
- `TriggerSearchRebuildPayload`: initiation of search document reindexing across source modules.
- `TrackSearchClickPayload`: telemetry recording of user clicks from search preview results.

### Domain Invariants
- Symmetric FFA boundary: `core/` contains 0 UI framework imports (`leptos::*`, `dioxus::*`); all formatting, view model mappings, and grid definitions are pure Rust functions verifiable via standard unit tests.
- UI components in `ui/leptos` contain zero business rules, domain validations, or direct database access.
- Tenant isolation: All admin queries and mutations require valid tenant context; cross-tenant access is rejected with `ApiError::Forbidden`.
- Dual-path parity: Both `NativeServer` and `Graphql` transport adapters produce identical DTO payloads.

### Events / Outbox Side Effects
- Dispatches search index rebuild jobs for background worker processing.
- Logs search analytics click events and records administrative setting mutations.

### Errors / Failure Codes
- `ApiError::Unauthorized`: Missing or invalid authentication token.
- `ApiError::Forbidden`: Tenant mismatch or insufficient permissions (`SEARCH_SETTINGS_*`).
- `ApiError::Validation`: Malformed JSON configuration, invalid ranking profile, or duplicate synonym/rule.
- `ApiError::NotFound`: Requested search configuration, dictionary entry, or tenant does not exist.
- `ApiError::Internal`: Unhandled server or transport communication error.
