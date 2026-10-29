# Crate API: rustok-commerce-admin

## Public surface

### UI Components
- `CommerceAdmin`: Root Leptos administration component for commerce operations and shipping profiles.

### Core
- Shipping profile forms, draft validation, and status badge resolution.
- Cart promotion display models and order change summary mapping.

### Transport
- GraphQL and native server adapters for shipping profile CRUD, cart promotions, and post-order change workflows.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-commerce`.
- Native server functions with `HostRuntimeContext` and transactional event bus integration.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-commerce` GraphQL surface.
- Cooperates with `rustok-cart` for promotion previews and adjustment views.
- Collaborates with `rustok-order` for post-order change lifecycle management.
- Coordinates with `rustok-fulfillment` for shipping zone and profile rules.

## Minimum Contract Set

### Input DTOs/Commands
- `CommerceAdmin`: Entrypoint administration workspace component.
- Shipping profile creation, update, deactivation, and reactivation commands.
- Order change apply and cancel commands.

### Domain Invariants
- Shipping profile rates and constraints must be strictly non-negative.
- Zero business logic inside UI view templates; draft preparation and mapping live in `core`.
- Post-order changes must preserve transactional consistency across order and fulfillment boundaries.

### Events / Outbox Side Effects
- Emits order change execution and shipping profile mutation requests to underlying services.

### Errors / Failure Codes
- Form validation errors displayed next to corresponding fields.
- Backend and GraphQL errors mapped into accessible alert feedback without component crashes.

