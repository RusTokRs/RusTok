# rustok-fulfillment-admin

Leptos admin UI package for the `rustok-fulfillment` module.

## Responsibilities

- Exposes the fulfillment admin root view used by `apps/admin`.
- Keeps shipping-option list/create/edit/lifecycle workflow inside the fulfillment-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses registry-backed shipping-profile selection so operators work with typed `allowed_shipping_profile_slugs` bindings instead of raw slug text.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `FulfillmentAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract for shipping-option CRUD while UI ownership moves to the fulfillment module.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
