# rustok-commerce-admin

Leptos admin UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce admin root view used by `apps/admin`.
- Keeps product-catalog and shipping-option operator UX inside the module package.
- Participates in the manifest-driven admin composition path through `rustok-module.toml`.
- Owns the GraphQL-driven product list/create/edit/publish/archive workflow plus typed shipping-option list/create/edit workflow for the module-owned commerce surface.

## Entry Points

- `CommerceAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Uses the `rustok-commerce` GraphQL contract plus shared auth hooks from `leptos-auth`.
- Consumes `shippingOptions`, `shippingOption`, `createShippingOption`, and `updateShippingOption` in addition to the product catalog GraphQL contract.
- Should remain compatible with the host `/modules/{module_slug}` contract and generic shell.

## Documentation

- See [platform docs](../../../docs/index.md).
