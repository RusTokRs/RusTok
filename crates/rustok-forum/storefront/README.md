# rustok-forum-storefront

Leptos storefront UI package for the `rustok-forum` module.

## Responsibilities

- Exposes the forum storefront root view used by `apps/storefront`.
- Keeps forum-specific storefront UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns dual-path storefront data access for category feeds, selected threads, and reply rendering.
- Adds native Leptos `#[server]` calls while keeping GraphQL as a required parallel fallback.
- Presents the module as a NodeBB-inspired public discussion surface.
- Ships package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.

## Entry Points

- `ForumView` - root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses native-first `#[server]` calls with GraphQL fallback and shared host libraries such as `UiRouteContext`.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.
- Reads the effective locale from `UiRouteContext.locale` for chrome copy and non-markdown rich-content summaries.

## Documentation

- See [platform docs](../../../docs/index.md).
