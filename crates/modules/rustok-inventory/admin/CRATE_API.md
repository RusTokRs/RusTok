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
- Reserve/release request quantities are non-negative; owner results retain legitimate signed
  negative `continue` availability. Set/adjust rules remain Inventory-owned, never UI clamping.
- Set-quantity operations preserve existing reservations.
- Zero business logic in UI view templates; calculations and mapping live in `core`.

### Events / Outbox Side Effects
- Emits inventory level updates and reservation state changes.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without crashing views.
- Server function errors mapped into user-visible alert notifications.

## Anonymous quantity controls (2026-11-01; source only)

Existing native reserve/release endpoints and DTOs are unchanged. Their mutation target is the
proved anonymous Inventory pool only, not a Cart/Order/line allocation. Native server functions
retain tenant/permission admission and delegate to `InventoryService`; owner failures remain
failures. The UI and both Fluent bundles explicitly label anonymous operations. Metadata,
original warehouse and other consumers' claims are not reassigned by quantity. Repeated successful
quantity commands are not idempotent and transport metadata is not a receipt.

See the [owner API](../CRATE_API.md#anonymous-quantity-reservations-2026-11-01-source-only).
That anonymous-pool prerequisite added no GraphQL fallback, Next.js mutation, receipt/schema
or migration. Its 27 Rust/10 Node definitions remain uncompiled/unexecuted. The subsequent
reservation-fact continuation forwards current context as described below; real Search/Index
convergence and Product step 8 remain pending.

## Current reservation context (2026-11-01; source only)

The existing reserve/release native endpoints build the service's `&PortContext` from current
`AuthContext`/`TenantContext`, correlation and optional request locale/channel. A conflicting
request tenant refuses; a missing optional locale/channel is not reconstructed from stored
reservation metadata. The two-second positive admission hint and diagnostic key do not create
idempotency or wall-clock cancellation proof. No browser/native DTO or route changes.
Inventory, not this UI/host, publishes the quantity-free fact on its mutation transaction.
See the [owner contract](../CRATE_API.md#reservation-invalidation-facts-2026-11-01-source-only)
for replay/zero and rollback behavior. Compilation/native/SSR/hydrate/mounted evidence is open.
