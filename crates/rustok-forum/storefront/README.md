# rustok-forum-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

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

## FFA boundary

The package keeps runtime-independent storefront policy in `src/core.rs`: route hrefs, rich-content summaries, count/slug labels, category/topic card view-models, accent fallback, and stable status badge class mapping. `src/transport.rs` remains the native-first/GraphQL fallback facade, while `src/ui/leptos.rs` is the explicit Leptos adapter. The fast non-compiling guardrail is `npm run verify:forum:storefront-boundary`.
