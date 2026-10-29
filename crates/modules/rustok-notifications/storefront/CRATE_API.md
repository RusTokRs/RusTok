# rustok-notifications-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for the notification inbox and navigation badge.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-notifications` module over native server functions and GraphQL.
- Collaborates with `apps/storefront` via header action slots and route mount.

## Public surface
- `pub mod core`: Pure Rust view-models, snapshots, and state representations.
- `pub mod transport`: Transport context, facades, and server adapters.
- `pub mod ui`: Leptos views and navigation badges (`NotificationsView`, `NotificationNavigation`, `NotificationUnreadBadge`).
- Primary types exported at crate root: `NotificationStorefrontState`, `NotificationStorefrontItem`, `NotificationStorefrontGroupSummary`, `NotificationStorefrontGroupSummaryPage`, `NotificationStorefrontGroupItemsPage`, `NotificationStorefrontInboxSnapshot`, `NotificationStorefrontGroupItemsSnapshot`.

## Minimum Contract Set

### Input DTOs/Commands
- `NotificationStorefrontGroupSummaryRequest`: Cursor-based group summary request.
- `NotificationStorefrontGroupItemsRequest`: Exact-group item pagination request.
- `NotificationStorefrontOpenRequest`: Fresh notification open authorization request.
- `NotificationStorefrontGroupStateCommand`: Idempotent group state mutation command.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Authoritative refresh: mutations refresh from owner backend without optimistic state modification.
- Single expanded group at a time.
- Group state mutations bounded to at most 64 eligible rows per action execution.

### Events / Outbox Side Effects
- Storefront UI adapter; state mutations delegate event publishing to `rustok-notifications`.

### Errors / Failure Codes
- Validation, auth, and network failures are captured as typed errors and rendered gracefully.
