# rustok-blog-storefront

Leptos storefront UI package for the `rustok-blog` module.

## Responsibilities

- Exposes the blog storefront root view used by `apps/storefront`.
- Keeps blog-specific storefront UI inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns dual-path read access for published posts and selected `?slug=` rendering.
- Native Leptos `#[server]` calls are added as the internal path, with GraphQL kept as a required parallel fallback.

## Entry Points

- `BlogView` — root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses native `#[server] -> PostService -> DB` on the SSR path and falls back to the `rustok-blog` GraphQL contract when native transport is unavailable.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.

## Documentation

- See [platform docs](../../../docs/index.md).
