# RusToK Page Builder Storefront

Leptos storefront renderer for Page Builder documents with an optional authenticated inline editor.

## Responsibilities

- Render canonical Page Builder documents for storefront routes via `PageBuilderStorefront`.
- Render pre-compiled immutable static landing artifacts directly via `StaticLandingStorefront` (direct SSR HTML and scoped CSS without `<iframe>`).
- CSS scoping: every surface that emits project CSS (`PageBuilderStorefront`, `StaticLandingStorefront`, `LocalizedPageBuilderStorefront`, and the inline editor) wraps it in a native CSS `@scope` rule and remaps the document-level selectors (`body`, `html`, `:root`) to `:scope`, so landing styles do not reach the host shell, navbar, or footer. The scope root is the landing body element the surface renders itself (`LANDING_SCOPE_SELECTOR`, `[data-fly-page-body]`), not the `class` a host may replace. The container's `isolation: isolate; contain: style layout;` gives it its own stacking context and containing block; it does not confine CSS rules.
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
- `scope_landing_css` / `LANDING_SCOPE_SELECTOR` (`src/css_scope.rs`) — pure CSS scoping and root-selector remapping. It is the Rust twin of `scopeLandingCss` in `apps/next-frontend/packages/rustok-pages`; both are asserted against the shared vectors in `../contracts/landing-css-scope-parity.json` and must change together.
- `FLY_STOREFRONT_BRIDGE_JS` — browser bridge for `fly:action` cart and navigation events.
- `fly_browser` — re-export of Fly browser and action assets (`FLY_ACTIONS_JS`).
- `src/inline_edit.rs` — feature-gated editor integration.

## Known Limitations / Pending Implementation

- **No fallback for browsers without `@scope`.** It needs Chrome/Edge 118+, Safari 17.4+ or Firefox 128+. An older browser drops the whole block: the landing renders unstyled there (the host is unaffected). There is no selector-prefixing fallback.
- **At-rules that define document-wide names are not confined.** `@keyframes`, `@font-face`, `@property` and similar keep their names global where a browser accepts them inside `@scope`; a landing can still collide with a host animation or font name.
- **Landing markup is not scoped, only its stylesheet.** The body HTML is emitted as produced by the Fly sanitizer (`inner_html`); `scope_landing_css` adds no second sanitization pass.

See the [Page Builder documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).

For the formal Rust API contract and guarantees, see [`CRATE_API.md`](CRATE_API.md).
