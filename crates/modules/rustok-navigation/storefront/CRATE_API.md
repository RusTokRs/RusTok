# Crate API: rustok-navigation-storefront

## Public surface
### Models
- `StorefrontMenu`: Represents an active navigation menu tree.
- `StorefrontMenuItem`: Represents an individual menu item, optionally with nested child items.
- `StorefrontMenuLocation`: Enum of supported menu slots (`Header`, `Footer`, `Sidebar`, `Mobile`).
- `StorefrontNavigationSnapshot`: Container for header and footer active menus.

### Transport
- `fetch_active_menu`: Fetches the active menu for a specific layout location and locale.
- `NavigationTransportError`: Typed error representing transport failures.

### UI Components
- `NavigationHeaderMenu`: Top-level storefront navigation menu component.
- `NavigationSnapshotProvider`: Context provider injecting navigation hierarchy into the DOM tree.
- `NavigationView`: View renderer for a specific navigation menu location.

## Platform Capability Bindings
- **`rustok-ui-i18n`**: Localized string lookups for navigation chrome.
- **`rustok-ui-core`**: Text normalization utilities.
- **`rustok-ui-transport`**: Dual transport execution via `execute_selected_transport`.

## Domain Collaborations
- **`rustok-navigation`**: Consumes backend menu bindings and localized items.
- **`rustok-channel`**: Binds active navigation slots per sales channel.
- **`rustok-tenant`**: Scopes navigation menus to the authenticated tenant.

## Minimum Contract Set
### Input DTOs/Commands
- `StorefrontMenuLocation`: Request parameter for menu slot selection (`header`, `footer`, `sidebar`, `mobile`).
- Locale string parameter (`en`, `ru`, etc.) passed to `fetch_active_menu`.

### Domain Invariants
- **Fluid Frontend Architecture (FFA)**: Strict separation between pure Rust navigation tree data models (`model`) and Leptos UI components (`ui`).
- **Framework-Agnostic Core Models**: `StorefrontMenu`, `StorefrontMenuItem`, and `StorefrontNavigationSnapshot` are fully serializable and testable without UI framework runtimes.
- **Dual-Path Transport**: Supports both direct native SSR server functions and headless GraphQL queries through `fetch_active_menu`.
- **Zero Fallbacks**: Transport errors are typed and propagated via `NavigationTransportError`; no silent fallbacks.

### Events / Outbox Side Effects
- Client-side navigation rendering produces no direct backend outbox side effects.

### Errors / Failure Codes
- `NavigationTransportError::Network`: Network or connectivity error during menu fetch.
- `NavigationTransportError::Serialization`: Deserialization or payload structure error.
- `NavigationTransportError::NotFound`: Requested navigation location has no published menu.
- `NavigationTransportError::Unauthorized`: Insufficient permissions to view channel navigation.
