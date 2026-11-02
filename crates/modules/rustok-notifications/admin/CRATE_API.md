# rustok-notifications-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for notifications administration.
- Integrates with `rustok-ui-core` and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-notifications` module capabilities.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models and notification status presentation models.
- `pub mod ui`: Leptos admin page component (`NotificationsAdmin`).
- `pub use ui::leptos::NotificationsAdmin`: Root admin view component.

## Minimum Contract Set

### Input DTOs/Commands
- Notification source registry status view models.
- Localized notification channel and status filter representations.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Notification admin surface exposes explicit source-registry status without synthesizing unverified delivery metrics.

### Events / Outbox Side Effects
- UI adapter package; does not dispatch events directly.

### Errors / Failure Codes
- Notification admin errors are captured and presented through Leptos UI error states.
