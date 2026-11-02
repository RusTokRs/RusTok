# rustok-payment-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for payment handoff and collection views.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-payment` for payment collection lifecycle and refund handling.
- Collaborates with `rustok-commerce-storefront` for checkout payment handoff.
- Collaborates with `apps/storefront` as a registered storefront module package.

## Public surface
- `pub mod core`: Pure Rust view-models and action-label policy.
- `pub mod transport`: Request DTOs, server function adapters, and normalization.
- `pub use ui::{PaymentCollectionActionButton, PaymentCollectionCard, PaymentView}`

## Minimum Contract Set

### Input DTOs/Commands
- Payment collection request DTOs and command metadata.
- Payment view models carrying localized status, amounts, and provider handoff links.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Payment presentation emits typed request DTOs instead of raw cart/order IDs.
- Locale negotiation is delegated to the host application context.

### Events / Outbox Side Effects
- Storefront UI adapter; server function handlers delegate transactional event publication to `rustok-payment`.

### Errors / Failure Codes
- Payment processing and transport errors are surfaced through typed result wrappers.
