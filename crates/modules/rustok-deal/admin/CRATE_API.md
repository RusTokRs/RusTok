# Crate API: rustok-deal-admin

## Public surface

### Core
- `DealAdminTableRow`: Table row representation for CRM deals data grid.
- `filter_rows`: Pure in-memory filtering function for deal records by title or status.

### Transport
- `DealAdminTransport`: Decoupled transport facade for deal administration endpoints.

### UI Components
- `DealAdmin`: View component mounting the deals administration workspace.
- `DealAdminPage`: Full-page view component for deals management.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-deal`.
- Data grid display via `rustok-grid` and `rustok-grid-leptos`.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-deal` domain services for deal pipelines and stage management.
- Coordinates with customer and auth boundaries for deal ownership and permissions.

## Minimum Contract Set

### Input DTOs/Commands
- `DealAdminTableRow`: Input row model for data grid rendering.
- `filter_rows`: Pure query filter taking row slice and search keyword.

### Domain Invariants
- Zero UI framework dependencies inside `core`; all filtering and row mapping are pure Rust.
- Deal stage transitions and pipeline validations preserve domain constraints.
- Transport facade encapsulates network execution without exposing raw transport details.

### Events / Outbox Side Effects
- Emits deal stage changes and pipeline modification actions through transport facade.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without throwing panics.
- Transport failures surface user feedback via error banners.

