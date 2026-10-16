# @rustok/pages-frontend

> Storefront package for the `rustok-pages` module in the Next.js storefront (`apps/next-frontend`).

## Responsibilities

- Render published Page Builder landing pages directly via SSR without an `<iframe>`.
- Parse compiled static landing artifacts (`parseArtifactHtml`), separating scoped CSS (`<style>`) and sanitized document markup (`<body>`).
- Provide an Islands Architecture bridge (`FlyActionBridge`) that listens for `fly:action` CustomEvents dispatched by interactive elements and hooks into platform capabilities (e.g. `@rustok/cart-frontend` `addItem` / `openCart`, Next.js client-side link interception).
- Resolve published pages via GraphQL queries (`fetchPublishedPage`) and fetch compiled static landing artifacts (`fetchPageArtifact`).

## Architecture & Rendering Model

Published Page Builder landing artifacts in RusToK are pre-compiled and sanitized by the Page Builder compiler into static landing artifacts stored in PostgreSQL (`page_static_landing_artifacts`).

Rather than isolating the public storefront within a sandboxed `<iframe>`, `@rustok/pages-frontend` performs direct SSR HTML and CSS injection:

1. **Server Component (`PageBuilderContent`):**
   - Automatically scopes the stylesheet via `scopeLandingCss` using modern CSS `@scope (.fly-page-container)` and remaps root selectors (`body`, `html`) to `:scope`.
   - Isolates styles and layout via `style={{ isolation: "isolate", contain: "style layout" }}` so builder styles never leak into the host shell, navbar, or footer.
   - Injects the scoped stylesheet `<style>` into the document render stream.
   - Injects the sanitized HTML into `<div className="fly-page-body" dangerouslySetInnerHTML={{ __html: html }} />`.
   - Mounts the client island `<FlyActionBridge />`.

2. **Client Island (`FlyActionBridge` & `fly-actions-runtime`):**
   - Initializes the lightweight, zero-dependency `fly-actions` runtime (`initFlyActions`).
   - Natively handles DOM actions: `toggle_visibility`, `scroll_to`, `animate`.
   - Bridges `emit_event` and `provider_action` events (`add_to_cart`, `buy_now`, `navigate`) to React context and Next.js router.
   - Intercepts internal same-origin link clicks for SPA client routing.

## Exports

- `PageBuilderContent` — React Component rendering scoped HTML + CSS + client action bridge.
- `FlyActionBridge` — Client Component handling interactive actions and cart integration.
- `scopeLandingCss` — Utility wrapping styles with `@scope (.fly-page-container)` and remapping root selectors.
- `initFlyActions` — Standalone idempotent actions runtime initializer.
- `parseArtifactHtml` — Utility extracting `<style>` and `<body>` from artifact documents.
- `fetchPublishedPage`, `fetchPageArtifact` — GraphQL and HTTP artifact fetching helpers.
