# rustok-calls-admin / CRATE_API

## Public surface

### Entry Point & Admin Pages
- `CallsAdmin`: root administration component for WebRTC calls and rooms.
- `CallsAdminPage`: page component hosting call sessions and conference room grids.

### Core Models & Logic
- `CallsAdminTableRow`: view model representing call room and active session records.
- `filter_rows`: in-memory search and filter predicate for calls table rows.
- `CallsAdminTransport`: transport facade managing backend communication.

## Platform Capability Bindings
- **Admin Control Plane**: Registers `CallsAdmin` within administrator workspaces.
- **Fluid Frontend Architecture**: Enforces clean boundary between Leptos components and pure Rust models in `core`.
- **Conference & Call Management**: Manages WebRTC rooms, peer connections, and signaling statuses.

## Domain Collaborations
- **`rustok-calls`**: Core domain module providing call session logic, room lifecycle, and signaling.
- **`rustok-grid-leptos`**: Data grid adapter presenting call rooms with pagination and sorting.
- **`rustok-ui-core`**: Core UI layout and accessibility foundations.
- **`rustok-ui-i18n`**: Fluent translation catalogs (`en.ftl`, `ru.ftl`) synchronized with host locale context.

## Minimum Contract Set

### Input DTOs/Commands
- `CallsAdminTableRow`: constructor parameters (`id`, `title`, `status`, `created_at`).
- `filter_rows`: filter predicate inputs (`rows`, `query`).

### Domain Invariants
- **Non-Swallowed Errors**: Signaling and room administration failures must surface operator diagnostics.
- **Room Isolation**: Conference rooms and signaling states are strictly isolated by tenant boundary.
- **Connection Hygiene**: Inactive or expired call sessions transition to terminated state promptly.

### Events / Outbox Side Effects
- Dispatches call session creation, room locking, and participant management through the transport facade.

### Errors / Failure Codes
- `UiError`: presentation and input validation error states.
- `CallsTransportError`: network and signaling failure codes for calls administration.

