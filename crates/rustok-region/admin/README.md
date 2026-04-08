# rustok-region-admin

Leptos admin UI package for the `rustok-region` module.

## Responsibilities

- Exposes the region operations admin root view used by `apps/admin`.
- Keeps region list/detail/create/update UX inside the region-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses native Leptos server functions as the primary admin transport instead of routing region CRUD back through the commerce umbrella.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `RegionAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads and writes through `rustok_region::RegionService`.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
