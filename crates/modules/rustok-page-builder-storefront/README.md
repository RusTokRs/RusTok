# RusToK Page Builder Storefront

Leptos storefront renderer for Page Builder documents with an optional authenticated inline editor.

## Responsibilities

- Render canonical Page Builder documents for storefront routes.
- Resolve locale-aware routes.
- Mount Fly inline editing only when the feature and host authorization allow it.

## Interactions

The crate consumes `rustok-page-builder`, `fly`, and optionally `fly-leptos`. Storefront hosts provide route, locale, and authorization context.

## Entry points

- `src/lib.rs` — renderer exports.
- `src/inline_edit.rs` — feature-gated editor integration.

See the [Page Builder documentation](../rustok-page-builder/docs/README.md) and the [module UI guide](../../../docs/UI/module-package-implementation.md).
