# rustok-calendar-admin / CRATE_API

## Public surface

### Entry Point & Admin Pages
- `CalendarAdmin`: root administration component for calendar and appointments.
- `CalendarAdminPage`: page component hosting appointment and schedule grids.

### Core Models & Logic
- `CalendarAdminTableRow`: view model representing appointment and calendar records.
- `filter_rows`: in-memory search and filter predicate for calendar table rows.
- `CalendarAdminTransport`: transport facade managing backend communication.

## Platform Capability Bindings
- **Admin Control Plane**: Registers `CalendarAdmin` within administrator workspaces.
- **Fluid Frontend Architecture**: Enforces clean boundary between Leptos components and pure Rust models in `core`.
- **Appointment & Schedule Management**: Manages calendars, time slots, and appointment statuses.

## Domain Collaborations
- **`rustok-calendar`**: Core domain module providing calendar entities, bookings, and availability rules.
- **`rustok-grid-leptos`**: Data grid adapter presenting appointments with pagination and sorting.
- **`rustok-ui-core`**: Core UI layout and accessibility foundations.
- **`rustok-ui-i18n`**: Fluent translation catalogs (`en.ftl`, `ru.ftl`) synchronized with host locale context.

## Minimum Contract Set

### Input DTOs/Commands
- `CalendarAdminTableRow`: constructor parameters (`id`, `title`, `status`, `created_at`).
- `filter_rows`: filter predicate inputs (`rows`, `query`).

### Domain Invariants
- **Non-Swallowed Errors**: Calendar and booking errors must be surfaced to administrators rather than hidden.
- **Temporal Integrity**: Start times, end times, and duration constraints follow strict validation rules.
- **Tenant Scope Isolation**: All appointment and calendar views are strictly isolated to the current tenant.

### Events / Outbox Side Effects
- Dispatches appointment booking, rescheduling, and cancellation requests through the transport facade.

### Errors / Failure Codes
- `UiError`: presentation and input validation error states.
- `CalendarTransportError`: network and RPC failure codes for calendar operations.

