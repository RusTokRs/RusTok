# `@rustok/pages-frontend` API Specification

## 1. Overview & Ownership

`@rustok/pages-frontend` is the Next.js storefront client package for the `rustok-pages` module (`crates/modules/rustok-pages`). It provides direct SSR rendering of published Page Builder landing pages without relying on sandboxed `<iframe>` tags, isolates landing styles from the host shell via modern CSS `@scope`, and bridges DOM interactive elements to platform capabilities via the lightweight Fly Actions runtime and React islands.

- **Package Name**: `@rustok/pages-frontend`
- **Location**: `apps/next-frontend/packages/rustok-pages`
- **Owner**: `rustok-pages` domain module
- **Runtime Environment**: Next.js App Router (React 19, Server Components & Client Islands)

---

## 2. Public API Surface & Exports

### Components

- `PageBuilderContent({ html, css, className }: PageBuilderContentProps)`:
  - Renders the scoped landing page HTML markup and injects scoped CSS.
  - Wraps content in `<div className="fly-page-container" style={{ isolation: "isolate", contain: "style layout" }}>`.
  - Mounts the client `<FlyActionBridge />` island for handling interactive triggers.
- `FlyActionBridge()`:
  - Client component (`"use client"`) that side-effect imports the canonical `fly-actions.js` runtime asset (a byte-exact copy of `crates/ui/fly/browser/assets/fly-actions.js`, the same script the Rust storefront inlines) and listens for `fly:action` CustomEvents.
  - Bridges actions (`add_to_cart`, `cart:add`, `buy_now`, `cart:open`, `cart:close`, `cart:toggle`, `navigate`) to `@rustok/cart-frontend` and Next.js `useRouter`.
  - Dynamically updates cart counter islands (`[data-fly-island="cart_count"]`) and intercepts relative same-origin link clicks for client-side SPA routing.

### Data Fetching & Parsers

- `fetchPublishedPage(slug: string, locale?: string, tenantSlug?: string | null, tenantId?: string | null): Promise<PageDetail | null>`:
  - Executes `STOREFRONT_PAGE_QUERY` via `storefrontGraphql`: one read of the published page by slug that returns its id, version, status, effective locale, translation (title, slug, SEO fields) and `landing`.
  - `PageDetail.landing` is a `PublishedLanding { locale, html, css, artifactHash }` (`GqlPublishedLanding` on the server): the sanitized body markup and the stylesheet stored by the reviewed publish, as separate fields. It is `null` when the page has no published landing for the effective locale on the request channel; the server applies the tenant default-locale and platform fallback itself.
  - The page body (the authoring document) is not requested. The server returns it only to callers that may save or publish the document.
  - Returns `null` when no published page matches the slug. Throws when the response carries any GraphQL error.
- `STOREFRONT_PAGE_QUERY`, `readStorefrontPage(response)` (`src/api/page-query.ts`):
  - The query document and the pure reader of its response, kept free of host imports so that the unit tests exercise them directly. Not re-exported from the package entry point.
- `scopeLandingCss(rawCss: string, scopeSelector?: string): string`:
  - Wraps the stylesheet in `@scope (<scopeSelector>)` and maps the document-level selectors (`body`, `html`, `:root`) to `:scope` where they appear in selector position.
  - The result is always one complete `@scope` block: a `}` that would close the wrapper early is dropped and unclosed blocks, strings, comments and `url()` tokens are closed. Only the function's own output (one block for the same selector) passes through unchanged. The result never contains `</style` (written `<\/style`), so it cannot end the `<style>` element it is rendered into. At-rules that define document-wide names (`@keyframes`, `@font-face`, `@property`) are not confined by `@scope`.
  - Only CSS whitespace (space, tab, LF, CR, FF) is trimmed and treated as a separator; other Unicode whitespace, such as a no-break space, is part of a name, as it is for a browser.
  - Parity: the Leptos storefront's `scope_landing_css` (`crates/modules/rustok-page-builder/storefront/src/css_scope.rs`) returns the same text for the same stylesheet and selector. Both are asserted against `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json`.

---

## 3. Architecture & Rendering Model

### Direct SSR Landing Injection

Unlike legacy iframe-based embedding, `@rustok/pages-frontend` renders published builder landing pages directly into the Next.js storefront DOM:
1. **Host Shell Isolation**:
   - The wrapper element applies `style={{ isolation: "isolate", contain: "style layout" }}`: a separate stacking context, and fixed/absolute landing content is laid out against the container rather than the viewport. `contain` does not confine CSS rules; that is done by the `@scope` wrapper below.
2. **Modern CSS Scoping**:
   - `scopeLandingCss` encapsulates all CSS rules within `@scope (.fly-page-container)`.
   - Selectors targeting `body`, `html`, or `:root` are remapped to `:scope`, preserving the styling intended by the designer while guarding the outer document.
   - Browser support: `@scope` needs Chrome/Edge 118+, Safari 17.4+ or Firefox 128+. Older browsers drop the block, so the landing renders unstyled there; there is no selector-prefixing fallback.
3. **Client Islands (`FlyActionBridge`)**:
   - The document remains pure static HTML/CSS on initial load.
   - Interactive triggers (`data-fly-action`, `data-fly-island="cart_trigger"`, `data-fly-island="cart_count"`) are progressively enhanced by `FlyActionBridge`.

---

## 4. Universal Logic Dimensions

### 1. Preconditions & Bounds
- `fetchPublishedPage`: `tenantSlug` and `tenantId` are optional; `null` is sent as absent.
- `scopeLandingCss`: Returns `""` for empty or whitespace-only input and is idempotent for its own output. Unbalanced input (stray `}`, unclosed blocks, unterminated strings/comments/`url()`) still yields one complete `@scope` block.
- Animation durations in the `fly-actions` runtime asset are clamped between 50ms and 10,000ms.

### 2. Branch Completeness & Anti-Swallowing
- `fetchPublishedPage`: Throws typed errors containing backend GraphQL error messages when GraphQL execution fails.
- `fetchPublishedPage`: a response with GraphQL errors throws even when partial data came with it, so a landing lost to a resolver error (for example an artifact integrity failure) never reads as a page without a landing. There is no fallback content source.
- The storefront route (`app/[locale]/pages/[slug]`) maps a failed read, a missing page and a page without a landing to `notFound()`, and logs a failed read.
- Non-matching or invalid selectors in link clicks (e.g. invalid hash syntax) are caught and ignored safely without throwing runtime unhandled exceptions.

### 3. State Transition Integrity
- `FlyActionBridge` coordinates with `@rustok/cart-frontend` state: `addItem` returns a promise, opening the cart drawer on success.
- The storefront has no draft or preview state: a page is either published with a landing (rendered) or absent (`notFound()`).
- `generateMetadata` and the page component share one page read per request (React `cache`), so the metadata and the rendered landing describe the same snapshot.

### 4. Side-Effect Completeness
- Event listeners for `fly:action` and document `click` are cleanly unregistered in `FlyActionBridge`'s `useEffect` cleanup hook.
- The `fly-actions` runtime asset self-starts idempotently on import: it keys the installed listener adapter on a document symbol (`Symbol.for("fly.actions.adapter")`), so hydration re-imports or dual mounting attach delegation exactly once per document lifecycle, and it is SSR-safe (`typeof document` guards).

### 5. Calculation & Algorithmic Correctness
- `scopeLandingCss` tokenizes the stylesheet as the CSS Syntax specification does and follows the resulting block structure: `{}`/`()`/`[]` blocks, strings with backslash escapes (an unescaped newline ends a string, backslash + CR LF continues it), comments, names with escapes, hash/at-keyword/numeric tokens and `url()` tokens. A `url()` token is recognised only where the tokenizer starts one (`url(`, `u\72l(`), never inside a longer token (`#url(`, `@url(`, `myurl(`, `3url(`), and only CSS whitespace counts as whitespace. Root selectors are remapped only in selector position — not in declarations, attribute selectors, at-rule preludes (`@keyframes body`) or `url()` — and only on identifier boundaries (`tbody` or `.some-body` are never mistaken for `body`).
- Quantity parsing in `handleFlyAction` sanitizes string inputs to positive numbers (`Number(data.quantity) || 1`).

### 6. Order of Operations & Concurrency
- Same-origin internal link clicks are intercepted synchronously before navigation initiates, routing via Next.js client router without full page reloads.
- External links (`target="_blank"`), downloads, or explicit action triggers bypass SPA interception.

---

## 5. Verification & Testing

- **Contract Unit Tests**: `apps/next-frontend/packages/rustok-pages/src/pages-frontend.test.mjs` (16 unit tests, including the GraphQL landing selection checked against the server type, fly-actions asset parity and the CSS scoping vectors shared with the Leptos storefront, `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json`).
- **Execution Command**:
  ```bash
  npm --prefix apps/next-frontend run test:pages
  ```
- **Browser Check of CSS Confinement**: `apps/next-frontend/tests/e2e/landing-css-scope.spec.ts` (4 Playwright tests). The shared vectors and a list of unbalanced and hostile stylesheets are scoped by `scopeLandingCss`, emitted as the text of a `<style>` element and read back through the CSS object model: exactly one `@scope` rule, nothing outside the `<style>` element, host elements keep their own style; `body` / `html` / `:root` rules style the landing container.
  ```bash
  npm --prefix apps/next-frontend run test:e2e -- landing-css-scope
  ```
- **Fly Actions Asset Parity** (byte-exact copy of `crates/ui/fly/browser/assets/fly-actions.js` + supported action-kind set vs the Rust `ComponentAction` enum):
  ```bash
  npm --prefix apps/next-frontend run verify:fly-actions-parity
  ```
- **Type Checking**:
  ```bash
  npm run typecheck:next-frontend
  ```
- **Storefront Parity & Route Verification**:
  ```bash
  npm run verify:storefront:routes
  ```

