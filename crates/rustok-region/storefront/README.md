# rustok-region-storefront

Leptos storefront UI package for the `rustok-region` module.

## Responsibilities

- Exposes the region storefront root view used by `apps/storefront`.
- Keeps region-specific storefront UI inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Provides a public read-side route for region, country, currency, and tax baseline discovery.
- Uses native Leptos `#[server]` entry points in parallel with the existing GraphQL transport.
- Ships package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.

## Entry Points

- `RegionView` - root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses `RegionService` directly on the SSR path and falls back to the existing `storefrontRegions` GraphQL contract when native transport is unavailable.
- Reads the effective locale from `UiRouteContext.locale` and builds internal links through `UiRouteContext::module_route_base()`.

## Documentation

- See [platform docs](../../../docs/index.md).
