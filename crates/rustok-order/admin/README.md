# rustok-order-admin

Leptos admin UI package for the `rustok-order` module.

## Responsibilities

- Exposes the order operations admin root view used by `apps/admin`.
- Keeps order list/detail/lifecycle UX inside the order-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Consumes the existing `rustok-commerce` GraphQL order transport while UI ownership moves to the module boundary.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `OrderAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL order queries and lifecycle mutations in parallel with the ongoing ecommerce UI split.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
