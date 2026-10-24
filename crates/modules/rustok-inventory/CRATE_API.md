# rustok-inventory / CRATE_API

## Public Modules
- `entities`: SeaORM entity models for inventory items, inventory levels, stock locations, and reservations.
- `migrations`: SeaORM migration definitions for inventory tables.
- `module`: Core `InventoryModule` definition and platform registration.
- `ports`: Availability check and reservation port traits and snapshots.
- `return_restock`: Return restock logic and idempotency handling.
- `services`: Domain services for stock adjustments, reservations, and public channel inventory projections.
- `shipment_stock`: Shipment stock movement ports and consumption logic.

## Platform Capability Bindings
- `InventoryModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `inventory`.
- Dependencies: `product`.
- Permissions: `INVENTORY_CREATE`, `INVENTORY_READ`, `INVENTORY_UPDATE`, `INVENTORY_DELETE`, `INVENTORY_LIST`, `INVENTORY_MANAGE`.

## Domain Collaborations
- Default inventory submodule of the ecommerce family.
- Owns inventory stock levels, stock locations, and reservation records.
- Provides `InventoryReservationPort` and `InventoryReservationIdentityPort` for checkout reservation lifecycle.
- Provides `BootstrapService` for transactional initial stock creation during product/variant creation.
- Implements `StockLocationTranslationTargetProvider` for localized stock location titles.
- Leptos admin package in `admin/` (`rustok-inventory-admin`).

## Minimum Contract Set

### Input DTOs/Commands
- `InventoryAvailabilityRequest`: variant_id, requested_quantity, optional location_id.
- `InventoryReservationRequest`: variant_id, quantity, optional location_id.
- `InventoryIdentityReservationRequest`: reservation_id, external_id, variant_id, line_item_id, quantity, optional location_id.
- `InventoryReservationReleaseRequest`: variant_id, quantity, optional location_id.
- `InventoryIdentityReservationReleaseRequest`: reservation_id, external_id.
- `InitialInventory`: variant_id, quantity, optional location_id.
- `ReturnRestockLine`: variant_id, quantity, location_id, return_id.

### Domain Invariants
- Denies float arithmetic; stock quantities are exact signed/unsigned integers.
- Non-negative stock: available quantity bounds and saturating arithmetic (`saturating_sub` / `saturating_add`) prevent underflow.
- Idempotent reservations: `(reservation_id, external_id)` pairs uniquely identify durable reservations; re-reserving with identical parameters is replay-safe.
- Case-insensitive backorder policy: `continue` policy permits reservations beyond on-hand quantity.
- Tenant isolation enforced across all database queries, levels, and reservations.

### Events / Outbox Side Effects
- Emits stock level change events and low stock alerts across transactional outbox.
- Idempotency receipts recorded for durable reservation operations.

### Errors / Failure Codes
- `CommerceError::NotFound`: stock location, inventory item, or level not found.
- `CommerceError::Validation`: nil UUID, negative quantity, or invalid reservation parameters.
- `CommerceError::InsufficientStock`: requested quantity exceeds available stock when backorder is disabled.
- `CommerceError::Conflict`: reservation external_id conflict with mismatched parameters.
- `CommerceError::Database`: underlying storage failure.
