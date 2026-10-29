# Crate API: rustok-inventory-admin

## Public surface

### UI Components
- `InventoryAdmin`: Root Leptos administration component for stock and inventory management.

### Core
- Pure Rust view-models, stock-health indicators, and quantity change policies.

### Models
- Stock quantity snapshots, variant inventory records, and reservation tracking DTOs.

### Transport
- Native server function adapters for stock reads, quantity adjustments, and reservations.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-inventory`.
- Data grid integration via `rustok-grid` and `rustok-grid-leptos`.
- Native server functions with `HostRuntimeContext` and transactional event bus.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-inventory` backend services (`InventoryService`).
- Coordinates with `rustok-product` for catalog variant relationships.
- Coordinates with `rustok-outbox` for inventory adjustment event propagation.

## Minimum Contract Set

### Input DTOs/Commands
- `InventoryAdmin`: Entrypoint workspace component mounting inventory administration.
- Variant stock set, adjustment (+/-), reservation, and availability check commands.

### Domain Invariants
- Quantities and adjustments enforce non-negative stock availability invariants.
- Set-quantity operations preserve existing reservations.
- Zero business logic in UI view templates; calculations and mapping live in `core`.

### Events / Outbox Side Effects
- Emits inventory level updates and reservation state changes.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without crashing views.
- Server function errors mapped into user-visible alert notifications.

