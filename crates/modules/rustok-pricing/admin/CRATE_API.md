# rustok-pricing-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for the pricing administrative interface.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-pricing` runtime services over native `#[server]` functions and GraphQL.
- Collaborates with `rustok-channel` and `rustok-commerce-foundation` for channel options and pricing DTOs.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use ui::PricingAdmin`: Root Leptos admin page component for pricing operations.

## Minimum Contract Set

### Input DTOs/Commands
- `CurrentTenant`: Identity and slug of the authenticated tenant.
- `PricingChannelOption`: Channel identifier, slug, status, and default flags.
- `PricingPriceListOption`: Price list identifier, name, list type, and optional rule/adjustment metadata.
- `PricingAdminBootstrap`: Initial bootstrap bundle carrying tenant, channels, and price lists.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Pricing resolution validation: `currency_code` must be a valid 3-letter ASCII code; quantity must be >= 1.
- Price drafts enforce positive amounts and valid decimal precision.

### Events / Outbox Side Effects
- Admin pricing mutations emit transactional outbox events through the pricing module runtime when configured.

### Errors / Failure Codes
- Validation failures produce typed `PricingAdminRequestError`.
- Server errors are safely propagated through Leptos action signals.
