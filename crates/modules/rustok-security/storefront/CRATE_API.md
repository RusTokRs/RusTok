# rustok-security-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for security verification and account protection views.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-security` module backend.
- Collaborates with `apps/storefront` as a registered storefront module package.

## Public surface
- `pub mod core`: Pure Rust view-models (`SecurityStorefrontViewModel`).
- `pub mod transport`: Transport facade (`SecurityStorefrontTransport`).
- `pub mod ui`: Storefront presentation views (`SecurityStorefrontView`).
- `pub use core::SecurityStorefrontViewModel`
- `pub use transport::SecurityStorefrontTransport`
- `pub use ui::SecurityStorefrontView`

## Minimum Contract Set

### Input DTOs/Commands
- `SecurityStorefrontViewModel`: Presentation model holding page title and verification attributes.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Pure Rust view-models testable with standard unit tests.

### Events / Outbox Side Effects
- Storefront UI adapter; does not dispatch events directly.

### Errors / Failure Codes
- Security errors are captured and presented through user-facing storefront notices.
