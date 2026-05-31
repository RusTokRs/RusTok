# rustok-blog-storefront

Leptos storefront UI package for the `rustok-blog` module.

## Responsibilities

- Exposes the blog storefront root view used by `apps/storefront`.
- Keeps blog-specific storefront UI inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns dual-path read access for published posts and selected `?slug=` rendering.
- Keeps storefront shell copy, selected-post route/query state, fetch request state, and presentation view-model helpers in framework-agnostic `core` so Leptos remains a thin render/host-context adapter.
- Native Leptos `#[server]` calls are isolated in `transport/native_server_adapter.rs`, with GraphQL kept as a required parallel fallback in `transport/graphql_adapter.rs` behind the native-first facade.

## Entry Points

- `BlogView` — root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses native `#[server] -> PostService -> DB` on the SSR path through the native adapter and falls back to the `rustok-blog` GraphQL adapter when native transport is unavailable.
- Consumes the host-provided effective locale from `UiRouteContext` for shell copy, reads the stable selected-post query key `slug` through core-owned route state, and passes `BlogStorefrontFetchRequest` into transport adapters.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.

## Documentation

- See [platform docs](../../../docs/index.md).
