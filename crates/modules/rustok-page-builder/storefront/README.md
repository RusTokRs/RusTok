# RusToK Page Builder Storefront

Leptos storefront renderer for Page Builder documents with an optional authenticated inline editor.

## Responsibilities

- Render canonical Page Builder documents for storefront routes via `PageBuilderStorefront`.
- Render pre-compiled immutable static landing artifacts directly via `StaticLandingStorefront` (direct SSR HTML and scoped CSS without `<iframe>`).
- Inline lightweight `fly-actions.js` runtime when interactive actions (`data-fly-action`) are present.
- Resolve locale-aware routes.
- Mount Fly inline editing only when the feature and host authorization allow it.

## Interactions

The crate consumes `rustok-page-builder`, `fly`, `fly-browser`, and optionally `fly-leptos`. Storefront hosts provide route, locale, and authorization context.
Published pages use `StaticLandingStorefront` to eliminate `<iframe>` overhead on public storefronts while preserving full CSS containment and action behaviors.

## Entry points

- `PageBuilderStorefront` — dynamic/preview renderer from GrapesJS project data.
- `StaticLandingStorefront` — direct SSR renderer for pre-compiled landing artifact HTML and scoped CSS.
- `fly_browser` — re-export of Fly browser and action assets (`FLY_ACTIONS_JS`).
- `src/inline_edit.rs` — feature-gated editor integration.

See the [Page Builder documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
