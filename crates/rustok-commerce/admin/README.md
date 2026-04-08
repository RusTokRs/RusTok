# rustok-commerce-admin

Leptos admin UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce admin root view used by `apps/admin`.
- Acts as the transitional umbrella admin control room while ecommerce UI ownership is split by module boundaries.
- Keeps shipping-profile registry and shipping-option operator UX inside the commerce package.
- Participates in the manifest-driven admin composition path through `rustok-module.toml`.
- Temporarily still carries aggregate ecommerce operator flows, but product CRUD has already started moving into `rustok-product/admin`.
- Uses registry-backed selectors for `shipping_profile_slug` and `allowed_shipping_profile_slugs`, so operators no longer type raw profile slugs into product and shipping-option forms.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.

## Entry Points

- `CommerceAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract plus shared auth hooks from `leptos-auth`.
- Coexists with `rustok-product-admin` during the current UI split until product CRUD is fully removed from the aggregate commerce route.
- Consumes `shippingProfiles`, `shippingProfile`, `createShippingProfile`, `updateShippingProfile`, `deactivateShippingProfile`, `reactivateShippingProfile`, `shippingOptions`, `shippingOption`, `createShippingOption`, `updateShippingOption`, `deactivateShippingOption`, and `reactivateShippingOption` in addition to the product catalog GraphQL contract.
- Should remain compatible with the host `/modules/{module_slug}` contract and generic shell.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [platform docs](../../../docs/index.md).
