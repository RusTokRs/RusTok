# rustok-marketplace-seller-admin / CRATE_API

## Role

`rustok-marketplace-seller-admin` is the Leptos admin FFA package for the `rustok-marketplace-seller` module.
It provides seller profile directory views, detail records, onboarding approvals, member rosters, and event timeline panels.

## Public surface

### Public Modules & Primary Components

- `MarketplaceSellerAdmin`: Root administrative view component for marketplace seller management.
- `MarketplaceSellerEventHistoryPanel`: Audit and lifecycle event history component.
- `MarketplaceSellerEventTimeline`: Chronological event visualization component.

### Primary Public Types

- `MarketplaceSellerAdminTransportProfile`: Explicit transport selection (`Native`, `Graphql`).
- `MarketplaceSellerAdminTransportContext`: Execution context containing transport profile and auth credentials.
- `MarketplaceSellerAdminFilters`: Query filters for directory listing (search, status, onboarding status, pagination).
- `MarketplaceSellerAdminDirectory`: Paginated directory response containing seller list items.
- `MarketplaceSellerAdminListItem`: Concise seller projection for tabular displays.
- `MarketplaceSellerAdminRecord`: Complete seller record projection including statuses and timestamps.
- `MarketplaceSellerAdminMember`: Seller team member projection with role and invitation status.
- `MarketplaceSellerAdminDetail`: Combined detail record containing seller and members.
- `MarketplaceSellerAdminEvent`: Audit event entry with kind, provenance, and metadata.
- `MarketplaceSellerAdminEventHistory`: Collection of events associated with a seller.
- `MarketplaceSellerCreateDraft`, `MarketplaceSellerProfileDraft`: Seller creation and update payloads.
- `MarketplaceSellerMemberCreateDraft`, `MarketplaceSellerMemberUpdateDraft`: Member lifecycle payloads.
- `MarketplaceSellerAdminCommand`: Strongly typed administrative commands for mutation.
- `MarketplaceSellerAdminCommandResult`: Outcome of administrative command execution.
- `MarketplaceSellerAdminShell`: Computable view-model for administrative shell rendering.

### Primary Functions

- `build_marketplace_seller_admin_shell`: Construct localized shell header and metadata.
- `selected_transport_profile`: Resolve transport profile from configuration or route params.
- `load_marketplace_seller_directory`: Fetch paginated sellers via selected transport.
- `load_marketplace_seller_detail`: Fetch single seller details and members.
- `load_marketplace_seller_event_history`: Fetch seller audit trail.
- `execute_marketplace_seller_command`: Submit state-changing command via selected transport.

## Platform Capability Bindings

- **FFA Admin Extension**: Integrates into the RusToK Admin host via Leptos component trees and routes.
- **Dual Transport Integration**: Dispatches through native server calls or GraphQL operations without implicit fallback.
- **I18n Localization**: Resolves localized strings using `rustok-ui-i18n` dictionary bindings.

## Domain Collaborations

- **`rustok-marketplace-seller`**: Consumes backend seller domain services and types in SSR mode.
- **`rustok-graphql`**: Consumes GraphQL queries and mutations when operating in client-side or remote mode.
- **`rustok-ui-transport`**: Utilizes unified transport executor with strict profile isolation.

## Minimum Contract Set

### Input DTOs/Commands

- `MarketplaceSellerAdminFilters`: `search`, `status`, `onboarding_status`, `page`, `per_page`.
- `MarketplaceSellerAdminCommand`: `Create`, `UpdateProfile`, `SubmitOnboarding`, `ReviewOnboarding`, `Suspend`, `Reactivate`, `AddMember`, `UpdateMember`.
- `MarketplaceSellerAdminTransportContext`: `profile`, `access_token`, `tenant_slug`.

### Domain Invariants

- **Transport Boundary**: Never silently fall back between Native and GraphQL transports; explicit failure preservation.
- **FFA Clean Separation**: Framework-agnostic models and business logic reside in `core/` without Leptos dependencies.
- **Idempotency**: All mutation commands require an idempotency key to prevent duplicate lifecycle transitions.

### Events / Outbox Side Effects

- UI triggers domain commands that emit outbox events within the owning backend service (e.g. `seller.created`, `seller.activated`, `seller.suspended`).

### Errors / Failure Codes

- Transport failures return typed `UiTransportResult` errors preserving HTTP/GraphQL failure diagnostics.
- Validation failures surface field-level errors to the admin operator without data loss.
