# rustok-inventory-admin

Leptos admin UI package for the `rustok-inventory` module.

## Responsibilities

- Exposes the inventory operations admin root view used by `apps/admin`.
- Keeps inventory visibility and stock-health UX inside the inventory-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses the inventory-owned read facade in `src/core.rs`, `src/api.rs`, `src/native.rs`, and `src/transport.rs`, rendered through the explicit `src/ui/leptos.rs` adapter for current admin read-side access.
- Uses native Leptos server functions backed by `AdminInventoryReadService` as the primary inventory read transport.
- Starts the dedicated inventory write/validation split with native `inventory/variant/set-quantity`, `inventory/variant/adjust-quantity`, `inventory/variant/reserve-quantity`, `inventory/variant/release-reservation`, and `inventory/variant/check-availability` server-function endpoints; set/adjust endpoints return the inventory-owned `InventoryQuantityWriteResult { quantity, in_stock }`, reserve returns `InventoryReservationWriteResult { reserved_quantity, available_quantity, in_stock }`, release-reservation returns `InventoryReservationReleaseWriteResult { released_quantity, available_quantity, in_stock }`, availability checks return `InventoryAvailabilityCheckResult { available }`, set-quantity is exposed through the inventory-owned API facade and backed by `InventoryService::set_variant_quantity`, adjust-quantity exposes delta semantics through `InventoryService::adjust_variant_quantity` for the +/-1 operator controls, reserve-quantity exposes reservation semantics through `InventoryService::reserve`, release-reservation uses `InventoryService::release_reservation_quantity`, and check-availability uses `InventoryService::check_variant_availability` without GraphQL fallback.
- Keeps the existing commerce GraphQL access isolated inside the transitional `CommerceGraphqlInventoryReadAdapter` as a compatibility fallback only when the native read path is unavailable, while remaining dedicated inventory write parity is completed.
- Maps transitional GraphQL runtime failures into the inventory-owned `InventoryTransportError` so `ApiError` does not expose `GraphqlHttpError`.
- Enforces the boundary with `tests/boundary.rs`: GraphQL runtime markers are allowed only in `src/transport.rs`, and the crate root exports only the UI entry point.
- Keeps compatibility snapshot tests for the current inventory read model in `src/model.rs`, including list paging fields, localized detail copy, variant inventory fields, and price fields; `tests/boundary.rs` also checks source-level parity across backend DTOs, the native mapper, and the transitional GraphQL adapter.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `InventoryAdmin` - root admin view exported from `src/ui/leptos.rs` and rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads inventory product, variant, stock-health, reservation-aware available quantity, and localized-copy fields through the inventory-owned facade; native server functions are the primary path, while the underlying commerce GraphQL adapter is transitional, limited to native-unavailable fallback, and must stay private to the package transport boundary.
- Writes targeted variant stock quantities through the dedicated native inventory facade from the variant detail set-quantity and +/-1 adjustment controls; reserve-quantity and release-reservation are also exposed from detail UI as native inventory-owned write facades for reservation flows, and check-availability is exposed from the detail UI as a native inventory-owned validation facade. These paths return typed stock/reservation/availability results, have no GraphQL fallback, enforce tenant/permission checks server-side, and share the service-level non-negative requested quantity invariant with legacy availability callers; reservation release uses existing inventory levels and tracked reservation item rows, and does not create stock state on failed release attempts.
- Reads the effective UI locale from `UiRouteContext.locale`; inventory detail cards resolve localized product copy against that host-owned locale and only fall back when that locale is missing.

## Transitional adapter removal criteria

Remove `CommerceGraphqlInventoryReadAdapter` after inventory has native read parity and the remaining dedicated write transport beyond set/adjust/reserve/release/check-availability for:

- product id, slug/handle, status, title, and localized copy needed by inventory views;
- variant identity fields and shipping profile hints;
- inventory quantity, policy, reservation-aware availability, and stock-health fields;
- compatibility coverage for the current inventory admin read model, including the `src/model.rs` serde snapshots and `tests/boundary.rs` backend DTO/native mapper/transitional adapter parity check.

## Documentation

- See [platform docs](../../../docs/index.md).
