# rustok-inventory-admin

Leptos admin UI package for the `rustok-inventory` module.

## Responsibilities

- Exposes the inventory operations admin root view used by `apps/admin`.
- Keeps inventory visibility and stock-health UX inside the inventory-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses the inventory-owned read facade in `src/core.rs`, `src/api.rs`, `src/native.rs`, and `src/transport.rs`, rendered through the explicit `src/ui/leptos.rs` adapter for current admin read-side access.
- Uses native Leptos server functions backed by `AdminInventoryReadService` as the primary inventory read transport.
- Keeps the existing commerce GraphQL access isolated inside the transitional `CommerceGraphqlInventoryReadAdapter` as a compatibility fallback only when the native read path is unavailable, until dedicated inventory write transport is available.
- Maps transitional GraphQL runtime failures into the inventory-owned `InventoryTransportError` so `ApiError` does not expose `GraphqlHttpError`.
- Enforces the boundary with `tests/boundary.rs`: GraphQL runtime markers are allowed only in `src/transport.rs`, and the crate root exports only the UI entry point.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `InventoryAdmin` - root admin view exported from `src/ui/leptos.rs` and rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads inventory product, variant, stock-health, and localized-copy fields through the inventory-owned facade; native server functions are the primary path, while the underlying commerce GraphQL adapter is transitional, limited to native-unavailable fallback, and must stay private to the package transport boundary.
- Reads the effective UI locale from `UiRouteContext.locale`; inventory detail cards resolve localized product copy against that host-owned locale and only fall back when that locale is missing.

## Transitional adapter removal criteria

Remove `CommerceGraphqlInventoryReadAdapter` after inventory has dedicated write transport and native read parity for:

- product id, slug/handle, status, title, and localized copy needed by inventory views;
- variant identity fields and shipping profile hints;
- inventory quantity, policy, availability, and stock-health fields;
- compatibility coverage for the current inventory admin read model.

## Documentation

- See [platform docs](../../../docs/index.md).
