# rustok-marketplace-listing-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for marketplace listing management, publication, and terms history.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, `rustok-ui-i18n`, and `rustok-graphql`.

## Domain Collaborations
- Collaborates with `rustok-marketplace-listing` module backend for listing records, terms, and approval states.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models and transport profile selectors.
- `pub mod model`: Transport DTOs (`MarketplaceListingAdminListItem`, `MarketplaceListingAdminDirectory`, `MarketplaceListingAdminFilters`, `MarketplaceListingAdminTerms`, `MarketplaceListingAdminRecord`).
- Native and GraphQL transport facade (`MarketplaceListingAdminTransportContext`, `load_marketplace_listing_directory`).
- `pub use ui::leptos::MarketplaceListingAdmin`: Root admin view component.

## Minimum Contract Set

### Input DTOs/Commands
- `MarketplaceListingAdminFilters`: Filter and pagination parameters for listing directories.
- `MarketplaceListingAdminCommand`: Publication, approval, and terms update commands.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Listings link to validated seller ID and master variant ID.
- Approval status transitions follow strict marketplace moderation policies.

### Events / Outbox Side Effects
- Admin UI adapter package; listing actions delegate event publication to `rustok-marketplace-listing`.

### Errors / Failure Codes
- Transport and validation failures produce typed `UiTransportResult` error outcomes.

