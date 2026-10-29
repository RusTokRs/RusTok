# Crate API: rustok-fulfillment-storefront

## Public surface

### Models
- `StorefrontDeliveryGroup`: Delivery group grouping items by fulfillment origin.
- `StorefrontShippingOption`: Available shipping method with rate and delivery estimate.

### UI Components
- `FulfillmentShippingHandoffNotice`: Notice displayed during fulfillment handoff.
- `FulfillmentShippingSelectionPanel`: Interactive panel for selecting shipping options per delivery group.
- `FulfillmentView`: Root storefront fulfillment view.

### Core
- Request normalization and selection validation logic.

### Transport
- Selection command DTOs and dual-transport adapters (native server and GraphQL).

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for `rustok-fulfillment`.
- Compatible with Leptos hydration and SSR modes via `rustok-ui-core`, `rustok-ui-i18n`, and `rustok-ui-transport`.

## Domain Collaborations
- Collaborates with `rustok-commerce` during checkout orchestration.
- Coordinates with `rustok-fulfillment` domain service for shipping calculations and provider selection.

## Minimum Contract Set

### Input DTOs/Commands
- `FulfillmentShippingSelectionPanel`: View component taking delivery groups and selection callbacks.
- Shipping option selection commands dispatched per delivery group.

### Domain Invariants
- Each delivery group must have a valid selected shipping option before checkout can complete.
- Pricing and rate amounts must remain non-negative.
- Zero business logic inside UI view templates; validation and mapping live in `core`.

### Events / Outbox Side Effects
- Emits delivery method selections and updates cart checkout fulfillment state.

### Errors / Failure Codes
- Ineligible shipping options or invalid delivery addresses surfaced as accessible error messages.
- Transport failures display inline retry notices without crashing components.

