# rustok-customer-admin

Leptos admin UI package for the `rustok-customer` module.

## Responsibilities

- Exposes the customer operations admin root view used by `apps/admin`.
- Keeps customer list/detail/create/update UX inside the customer-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses native Leptos server functions as the primary admin transport instead of inventing a new umbrella GraphQL or REST layer.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `CustomerAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads and writes through `rustok-customer::CustomerService`, with optional profile enrichment through `rustok-profiles::ProfileService`.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
