# Crate API: rustok-brand-admin

## Minimum Contract Set
- **Fluid Frontend Architecture (FFA)**: Strict decoupling of domain logic, slug/name validation, and shell formatting in `core` from Leptos template rendering.
- **Pure Rust Domain Invariants**: Verification of brand slug and name rules via unit tests without Leptos or DOM runtime.
- **Dual-Path Transport**: Seamless support for both direct native SSR server functions and headless GraphQL transports via `BrandAdminTransportContext`.
- **Zero Fallbacks**: Explicit transport failure reporting without silent fallback masks.

## Platform Capability Bindings
- **`rustok-grid`**: Declarative grid columns for brand list with name search and active status filters (`brand_grid_columns`).
- **`rustok-ui-core`**: Text normalization via `normalize_ui_text`.
- **`rustok-ui-i18n`**: Bilingual support (`en`, `ru`) for brand admin shell titles and column headers.
- **`rustok-ui-transport`**: Dual transport execution via `execute_selected_transport`.

## Domain Collaborations
- **`rustok-brand`**: Interacts with the backend brand service, migrations, and DTO structures.
- **`rustok-product`**: Associates catalog products with brand manufacturers.

## Public Surface
### Core
- `BrandAdminTransportProfile`: Transport profile enum (`Native`, `Graphql`).
- `selected_transport_profile`: Resolves transport profile from string option.
- `build_brand_admin_shell`: Constructs localized shell metadata.
- `validate_brand_slug`: Validates brand slug format (alphanumeric, hyphens, underscores, max 100 chars).
- `validate_brand_name`: Validates non-empty brand name (max 255 chars).
- `brand_grid_columns`: Builds grid column definitions for brand table.

### Models
- `BrandAdminListItem`: Serialized brand summary item for directory tables.
- `BrandAdminDirectory`: Paginated collection of brand items.
- `BrandAdminFilters`: Filter parameters for directory queries.
- `BrandAdminTranslation`: Localized brand name and description.
- `BrandAdminRecord`: Detailed brand model including metadata and translations.
- `BrandAdminCreateDraft`: Draft for brand creation.
- `BrandAdminUpdateDraft`: Draft for updating existing brand fields.
- `BrandAdminCommand`: Admin command enum (`Create`, `Update`, `Delete`, `SetTranslation`).
- `BrandAdminCommandResult`: Result of executing an admin command.
- `BrandAdminShell`: Shell metadata for admin UI header.

### Transport
- `BrandAdminTransportContext`: Transport execution context holding profile, token, and tenant slug.
- `load_brand_directory`: Asynchronous loader for paginated brand directory.
- `load_brand_detail`: Asynchronous loader for single brand detail record.
- `execute_brand_command`: Asynchronous command executor for brand mutations.
- `BRAND_TRANSPORT_FALLBACK_POLICY`: Fallback policy declaration (`never falls back`).

### UI Components
- `BrandAdmin`: Standalone full-page admin view component for brand catalog management.
