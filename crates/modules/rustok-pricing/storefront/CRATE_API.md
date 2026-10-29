# rustok-pricing-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for pricing discovery.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-pricing::PricingService` over native `#[server]` functions and GraphQL.
- Collaborates with `rustok-channel` for available storefront sales channels.
- Collaborates with `apps/storefront` as a registered storefront module package.

## Public surface
- `pub use ui::leptos::PricingView`: Public storefront route component for pricing exploration.

## Minimum Contract Set

### Input DTOs/Commands
- `StorefrontPricingData`: Root DTO containing product pricing lists, selected detail, context, and channels.
- `PricingPriceListOption`: Active price list overlay with type and discount rules.
- `PricingChannelOption`: Public storefront channel with default and active flags.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Pricing resolution invariants: currency codes must be 3-letter ASCII codes; quantity must be >= 1.
- Effective price respects channel scope, quantity tiers, and price list fallback rules.

### Events / Outbox Side Effects
- Read-only storefront presentation package; does not emit domain events or mutate database rows.

### Errors / Failure Codes
- Validation and remote fetch errors are captured as typed errors and rendered gracefully in the storefront UI.
