# rustok-commerce-storefront

Leptos storefront UI package for the `rustok-commerce` module.

## Responsibilities

- Exposes the commerce storefront root view used by `apps/storefront`.
- Keeps catalog-specific public UX inside the module package.
- Participates in the manifest-driven storefront composition path through `rustok-module.toml`.
- Owns dual-path storefront data access for published products and selected product detail.
- Adds native Leptos `#[server]` calls in parallel with the existing GraphQL transport instead of replacing it.

## Entry Points

- `CommerceView` - root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses native-first `#[server]` calls with GraphQL fallback and stays compatible with the `rustok-commerce` storefront contract.
- Should remain compatible with the host storefront slot and generic module page contract.

## Documentation

- See [platform docs](../../../docs/index.md).
