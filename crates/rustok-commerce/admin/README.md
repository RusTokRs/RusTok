# rustok-commerce-admin

Leptos admin UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce admin root view used by `apps/admin`.
- Acts as the commerce-owned shipping-profile registry surface while ecommerce UI ownership is split by module boundaries.
- Keeps the typed shipping-profile registry inside the commerce package.
- Participates in the manifest-driven admin composition path through `rustok-module.toml`.
- No longer carries product CRUD; that catalog UI now lives in `rustok-product/admin`.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `CommerceAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract plus shared auth hooks from `leptos-auth`.
- Coexists with `rustok-product-admin` and `rustok-fulfillment-admin` during the current UI split while other ecommerce admin slices still move to their module-owned packages.
- Consumes `shippingProfiles`, `shippingProfile`, `createShippingProfile`, `updateShippingProfile`, `deactivateShippingProfile`, and `reactivateShippingProfile`.
- Should remain compatible with the host `/modules/{module_slug}` contract and generic shell.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
