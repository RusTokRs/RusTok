# RusToK Page Builder Storefront

Leptos storefront renderer for Page Builder documents with an optional authenticated inline editor.

## Responsibilities

- Render canonical Page Builder documents for storefront routes via `PageBuilderStorefront`.
- Render pre-compiled immutable static landing artifacts directly via `StaticLandingStorefront` (direct SSR HTML and scoped CSS without `<iframe>`).
- Automatic CSS Scoping and Containment: wraps user-generated CSS in native CSS `@scope (.rustok-page-builder-storefront)` and remaps root selectors (`body`, `html`) to `:scope`, paired with container `isolation: isolate; contain: style layout;` to eliminate style leakage into the host shell, navbar, and footer.
- Event and Navigation Bridge: inlines `FLY_STOREFRONT_BRIDGE_JS` to bridge `fly:action` CustomEvents (`add_to_cart`, `cart:add`, `buy_now`) to `rustok:cart:add` and handle in-page hash links.
- Inline lightweight `fly-actions.js` runtime when interactive actions (`data-fly-action`) are present.
- Resolve locale-aware routes.
- Mount Fly inline editing only when the feature and host authorization allow it.

## Interactions

The crate consumes `rustok-page-builder`, `fly`, `fly-browser`, and optionally `fly-leptos`. Storefront hosts provide route, locale, and authorization context.
Published pages use `StaticLandingStorefront` to eliminate `<iframe>` overhead on public storefronts while preserving full CSS containment and action behaviors.

## Entry points

- `PageBuilderStorefront` — dynamic/preview renderer from GrapesJS project data with scoped CSS.
- `StaticLandingStorefront` — direct SSR renderer for pre-compiled landing artifact HTML and scoped CSS.
- `scope_landing_css` — pure CSS scoping and root-selector remapping utility.
- `FLY_STOREFRONT_BRIDGE_JS` — browser bridge for `fly:action` cart and navigation events.
- `fly_browser` — re-export of Fly browser and action assets (`FLY_ACTIONS_JS`).
- `src/inline_edit.rs` — feature-gated editor integration.

See the [Page Builder documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).

For the formal Rust API contract and guarantees, see [`CRATE_API.md`](CRATE_API.md).
