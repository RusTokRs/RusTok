# Crate API: rustok-fulfillment-admin

## Public surface

### UI Components
- `FulfillmentAdmin`: Root Leptos administration component for shipping option management.

### Core
- Shipping option list defaults, pagination policies, and filter normalizers.

### Transport
- GraphQL transport facade for shipping option CRUD operations.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-fulfillment`.
- Data grid display via `rustok-grid` and `rustok-grid-leptos`.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-fulfillment` domain services for shipping option lifecycle and carrier zones.
- Coordinates with `rustok-commerce` shipping profile registries for allowed profile bindings.

## Minimum Contract Set

### Input DTOs/Commands
- `FulfillmentAdmin`: Entrypoint workspace component mounting fulfillment administration.
- Shipping option creation, update, and lifecycle commands.

### Domain Invariants
- Shipping options must bind to valid shipping profile slugs.
- Filter normalization and pagination defaults are pure Rust logic within `core`.
- Mutations preserve tenant and channel isolation boundaries.

### Events / Outbox Side Effects
- Emits shipping option configuration changes and status updates.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without throwing panics.
- GraphQL transport failures display user-visible error alerts.

