# rustok-order-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for order placement, checkout completion, and order history.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-order` for order domain records and history queries.
- Collaborates with `rustok-commerce-storefront` for checkout result and complete-checkout orchestration.
- Collaborates with `apps/storefront` as a registered storefront module package.

## Public surface
- `pub mod core`: Pure Rust checkout result handoff view-models and action-label policy.
- `pub mod model`: Storefront order data transfer objects (`StorefrontOrder`, `StorefrontOrderLineItem`, `StorefrontOrderAdjustment`, `StorefrontCustomerAddress`, `StorefrontOrdersResponse`).
- `pub mod transport`: Request DTOs, completion command metadata, and server adapters.
- `pub use ui::{OrderCheckoutCompleteButton, OrderCheckoutResultCard, OrderView, OrdersHistoryView}`

## Minimum Contract Set

### Input DTOs/Commands
- Complete checkout request DTO carrying cart ID, shipping/billing selection, and fulfillment preferences.
- `StorefrontOrder`: Order representation with line items, adjustments, status, and totals.
- `StorefrontOrdersResponse`: Paginated order history list.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Action components emit order-owned typed request DTOs instead of raw cart IDs.
- Presentation logic respects localized status mapping and order state machines.

### Events / Outbox Side Effects
- Storefront UI adapter; completion handlers delegate event publication to `rustok-order` and `rustok-commerce`.

### Errors / Failure Codes
- Order completion and query errors are safely mapped to user-facing error notices.
