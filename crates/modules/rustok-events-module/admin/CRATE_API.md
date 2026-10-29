# Crate API: rustok-events-admin

## Public surface

### UI Components
- `EventsAdmin`: Root Leptos administration component for event delivery profiles and event monitoring.

### Core
- Event presentation models, delivery profile filters, and status resolution helpers.

### Transport
- Dual-transport facade executing event administration queries and profile mutations.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-events`.
- Compatible with Leptos hydration and SSR modes via `rustok-ui-i18n` and `rustok-ui-transport`.

## Domain Collaborations
- Collaborates with `rustok-events` capability and `rustok-events-module` runtime hooks.
- Coordinates with `rustok-outbox` for delivery queue tracking and retry policies.

## Minimum Contract Set

### Input DTOs/Commands
- `EventsAdmin`: Entrypoint workspace component mounting events administration.
- Delivery profile updates and event retry dispatch commands.

### Domain Invariants
- Event delivery profiles preserve tenant isolation boundaries.
- Zero business logic inside UI view templates; state mapping lives in `core`.
- Mutations execute through selected transport without silent error masking.

### Events / Outbox Side Effects
- Emits delivery profile mutations and manually triggered event retry requests.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without panics.
- Transport failures display accessible error alert feedback.

