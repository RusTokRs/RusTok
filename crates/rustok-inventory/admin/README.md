# rustok-inventory-admin

Leptos admin UI package for the `rustok-inventory` module.

## Responsibilities

- Exposes the inventory operations admin root view used by `apps/admin`.
- Keeps inventory visibility and stock-health UX inside the inventory-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses the inventory-owned read facade in `src/core.rs`, `src/api.rs`, and `src/transport.rs` for current admin read-side access.
- Keeps the existing commerce GraphQL access isolated inside the transitional `CommerceGraphqlInventoryReadAdapter` until dedicated inventory transport is available.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `InventoryAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads inventory product, variant, stock-health, and localized-copy fields through the inventory-owned facade; the underlying commerce GraphQL adapter is transitional and must stay private to the package transport boundary.
- Reads the effective UI locale from `UiRouteContext.locale`; inventory detail cards resolve localized product copy against that host-owned locale and only fall back when that locale is missing.

## Transitional adapter removal criteria

Remove `CommerceGraphqlInventoryReadAdapter` after inventory has dedicated read/write transport for:

- product id, slug/handle, status, title, and localized copy needed by inventory views;
- variant identity fields and shipping profile hints;
- inventory quantity, policy, availability, and stock-health fields;
- compatibility coverage for the current inventory admin read model.

## Documentation

- See [platform docs](../../../docs/index.md).
