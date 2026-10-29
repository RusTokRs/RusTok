# rustok-calendar-storefront / CRATE_API

## Public surface

### Entry Point & Components
- `CalendarView`: mounted Leptos storefront component rendering public calendar and appointment booking.
- `CalendarViewModel`: view model representing appointment and calendar view states.
- `CalendarStorefrontTransport`: transport facade managing backend communication.

## Platform Capability Bindings
- **Storefront Route Integration**: Registers `CalendarView` for public appointment booking.
- **Fluid Frontend Architecture**: Strictly isolates Leptos rendering from framework-agnostic models in `core`.
- **Public Schedule Presentation**: Presents available booking slots and handles booking submissions.

## Domain Collaborations
- **`rustok-calendar`**: Core calendar module providing slot availability and booking rules.
- **`rustok-ui-core`**: Core UI layout and accessibility foundations.
- **`rustok-ui-i18n`**: Fluent translation catalogs (`en.ftl`, `ru.ftl`) synchronized with host locale context.

## Minimum Contract Set

### Input DTOs/Commands
- `CalendarViewModel`: constructor parameter (`title`).
- `CalendarStorefrontTransport`: constructor creating transport facade.

### Domain Invariants
- **Non-Swallowed Errors**: Booking submission or slot availability failures must surface user feedback.
- **Availability Enforcement**: Appointments can only be booked into valid, unreserved time slots.
- **Timezone Awareness**: Slot times are rendered in visitor's local context while persisted in UTC.

### Events / Outbox Side Effects
- Dispatches public appointment reservation requests through the backend transport facade.

### Errors / Failure Codes
- `UiError`: presentation and input validation error states.
- `CalendarTransportError`: network and RPC failure codes for storefront booking operations.

