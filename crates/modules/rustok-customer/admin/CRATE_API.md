# Crate API: rustok-customer-admin

## Public surface

### UI Components
- `CustomerAdmin`: Root Leptos administration component for customer operations.

### Core
- `customer_grid_columns`: Column definitions for the customer directory table.
- `filter_customers`: In-memory filtering helper for customer records.

### Models
- Customer summary, detailed profile records, and status representations.

### Transport
- Native server functions executing operations against `CustomerService`.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-customer`.
- Grid integration via `rustok-grid` and `rustok-grid-leptos`.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).
- Native server functions via `HostRuntimeContext`.

## Domain Collaborations
- Collaborates with `rustok-customer` backend service for customer lifecycle operations.
- Interacts with `rustok-profiles` for extended profile attribute enrichment.

## Minimum Contract Set

### Input DTOs/Commands
- `CustomerAdmin`: Entrypoint workspace component for customer administration.
- Customer creation, update, and profile attribute editing commands.

### Domain Invariants
- Customer email addresses and identifiers must preserve tenant boundary constraints.
- Zero business logic inside UI view templates; filtering, view-models, and validation live in `core`.
- Mutations execute through native server functions with proper authorization context.

### Events / Outbox Side Effects
- Emits customer profile updates and lifecycle state modifications.

### Errors / Failure Codes
- Form validation errors displayed next to input fields.
- Server function failures mapped to user-visible alert notifications.

