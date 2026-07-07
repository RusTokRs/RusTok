# rustok-inventory-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-inventory` module.

## Responsibilities

- Exposes the inventory operations admin root view used by `apps/admin`.
- Keeps inventory visibility and stock-health UX inside the inventory-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses inventory-owned `src/core.rs`, `src/transport/mod.rs`, explicit `src/transport/native_server_adapter.rs`, and `src/native.rs`, rendered through the explicit `src/ui/leptos.rs` adapter for current admin read-side access.
- Uses native Leptos server functions backed by `AdminInventoryReadService` as the only inventory admin read transport.
- Starts the dedicated inventory write/validation split with native `inventory/variant/set-quantity`, `inventory/variant/adjust-quantity`, `inventory/variant/reserve-quantity`, `inventory/variant/release-reservation`, and `inventory/variant/check-availability` server-function endpoints; set/adjust endpoints return the inventory-owned `InventoryQuantityWriteResult { quantity, in_stock }`, reserve returns `InventoryReservationWriteResult { reserved_quantity, available_quantity, in_stock }`, release-reservation returns `InventoryReservationReleaseWriteResult { released_quantity, available_quantity, in_stock }`, availability checks return `InventoryAvailabilityCheckResult { available }`, set-quantity is exposed through the inventory-owned transport facade and backed by `InventoryService::set_variant_quantity` as a reservation-aware available-quantity target that preserves existing reserved units, adjust-quantity exposes delta semantics through `InventoryService::adjust_variant_quantity` for the +/-1 operator controls, reserve-quantity exposes reservation semantics through `InventoryService::reserve`, release-reservation uses `InventoryService::release_reservation_quantity`, and check-availability uses `InventoryService::check_variant_availability` without GraphQL fallback.
- Enforces the boundary with `tests/boundary.rs`: GraphQL runtime markers, the old GraphQL `src/transport.rs` adapter, and the pre-FFA `src/api.rs` facade must be absent; the crate root exports only the UI entry point and keeps `transport/` private.
- Keeps compatibility snapshot tests for the current inventory read model in `src/model.rs`, including list paging fields, localized detail copy, variant inventory fields, and price fields; `tests/boundary.rs` also checks source-level parity across backend DTOs and the native mapper.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `InventoryAdmin` - root admin view exported from `src/ui/leptos.rs` and rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads inventory product, variant, stock-health, reservation-aware available quantity, and localized-copy fields through the inventory-owned transport facade; native server functions are the only read path, and the previous commerce GraphQL compatibility fallback has been removed from the package.
- Writes targeted variant stock quantities through the dedicated native inventory transport facade from the variant detail set-quantity and +/-1 adjustment controls; reserve-quantity and release-reservation are also exposed from detail UI as native inventory-owned write facades for reservation flows, and check-availability is exposed from the detail UI as a native inventory-owned validation facade. These paths return typed stock/reservation/availability results, have no GraphQL fallback, enforce tenant/permission checks server-side, and share the service-level non-negative requested quantity invariant with legacy availability callers; reservation release uses existing inventory levels and tracked reservation item rows, and does not create stock state on failed release attempts.
- Reads the effective UI locale from `UiRouteContext.locale`; inventory detail cards resolve localized product copy against that host-owned locale and only fall back when that locale is missing.

## Native-only transport status

`CommerceGraphqlInventoryReadAdapter` has been removed after native read parity for the current inventory admin read model. No GraphQL fallback remains in this package: `src/transport.rs`, `rustok-graphql`, token/tenant-slug fallback parameters, and GraphQL runtime error mapping are intentionally absent. Future inventory admin read/write expansion must add inventory-owned native/transport facade functions and targeted parity tests instead of reintroducing a commerce GraphQL adapter.

Current native read coverage includes:

- product id, slug/handle, status, title, and localized copy needed by inventory views;
- variant identity fields and shipping profile hints;
- inventory quantity, policy, reservation-aware availability, and stock-health fields;
- compatibility coverage for the current inventory admin read model through `src/model.rs` serde snapshots and `tests/boundary.rs` backend DTO/native mapper parity checks.

## Documentation

- See [platform docs](../../../docs/index.md).
