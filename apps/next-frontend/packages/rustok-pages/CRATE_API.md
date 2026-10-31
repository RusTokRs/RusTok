# `@rustok/pages-frontend` API Specification

## 1. Overview & Ownership

`@rustok/pages-frontend` is the Next.js storefront client package for the `rustok-pages` module (`crates/modules/rustok-pages`). It provides direct SSR rendering of published Page Builder landing pages without relying on sandboxed `<iframe>` tags, isolates landing styles from the host shell via modern CSS `@scope`, manages on-demand ISR revalidation tags, and bridges DOM interactive elements to platform capabilities via the lightweight Fly Actions runtime and React islands.

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
  - Client component (`"use client"`) that initializes `fly-actions-runtime` and listens for `fly:action` CustomEvents.
  - Bridges actions (`add_to_cart`, `cart:add`, `buy_now`, `cart:open`, `cart:close`, `cart:toggle`, `navigate`) to `@rustok/cart-frontend` and Next.js `useRouter`.
  - Dynamically updates cart counter islands (`[data-fly-island="cart_count"]`) and intercepts relative same-origin link clicks for client-side SPA routing.
- `DraftPreviewBanner({ slug, locale, exitUrl }: DraftPreviewBannerProps)`:
  - Sticky warning banner displayed at the top of the viewport when viewing an unpublished page in Next.js Draft Mode.
  - Provides a direct link to exit draft mode (`/api/disable-draft?redirect=...`).

### Data Fetching & Parsers

- `fetchPublishedPage(slug: string, locale?: string, tenantSlug?: string | null, tenantId?: string | null): Promise<PageDetail | null>`:
  - Executes `STOREFRONT_PAGE_QUERY` via `storefrontGraphql` to fetch published page metadata, translations, and body content by slug.
- `fetchPageArtifact(artifactUrlOrContent: string, format: string, baseUrlOrOptions?: string | FetchPageArtifactOptions): Promise<ArtifactContent>`:
  - Fetches and parses compiled static landing artifacts via HTTP or decodes inline HTML documents into `{ html, css }`.
- `parseArtifactHtml(documentHtml: string): ArtifactContent`:
  - Pure function extracting `<style>` blocks and inner `<body>` markup from compiled HTML documents.
- `pageCacheTags(slug?: string): string[]`:
  - Returns canonical Next.js cache tags (`["page-${slug}", "pages"]` or `["pages"]`) for on-demand ISR revalidation.
- `scopeLandingCss(rawCss: string, scopeSelector?: string): string`:
  - Lexical CSS parser and remapper that wraps declarations in `@scope (<scopeSelector>)` and normalizes root-level selectors (`body`, `html`, `:root`) to `:scope`.

---

## 3. Architecture & Rendering Model

### Direct SSR Landing Injection

Unlike legacy iframe-based embedding, `@rustok/pages-frontend` renders published builder landing pages directly into the Next.js storefront DOM:
1. **Host Shell Isolation**:
   - The wrapper element applies `style={{ isolation: "isolate", contain: "style layout" }}`.
   - CSS rules from Page Builder templates cannot leak into storefront navigation headers, footer links, or modals.
2. **Modern CSS Scoping**:
   - `scopeLandingCss` encapsulates all CSS rules within `@scope (.fly-page-container)`.
   - Selectors targeting `body`, `html`, or `:root` are safely remapped to `:scope`, preserving full visual styling intended by the designer while guarding the outer document.
3. **Client Islands (`FlyActionBridge`)**:
   - The document remains pure static HTML/CSS on initial load.
   - Interactive triggers (`data-fly-action`, `data-fly-island="cart_trigger"`, `data-fly-island="cart_count"`) are progressively enhanced by `FlyActionBridge`.

---

## 4. Universal Logic Dimensions

### 1. Preconditions & Bounds
- `pageCacheTags`: Handles undefined, null, or empty string slugs gracefully by falling back to `["pages"]`.
- `scopeLandingCss`: Handles empty, whitespace-only, or already-scoped CSS strings idempotently without malformed syntax.
- `fetchPageArtifact`: Revalidation durations are clamped and validated; defaults to 60s when not specified.
- Animation durations in `fly-actions-runtime` are clamped between 50ms and 10,000ms.

### 2. Branch Completeness & Anti-Swallowing
- `fetchPublishedPage`: Throws typed errors containing backend GraphQL error messages when GraphQL execution fails.
- `fetchPageArtifact`: Verifies HTTP response status (`res.ok`) and throws descriptive error on HTTP failures.
- Non-matching or invalid selectors in link clicks (e.g. invalid hash syntax) are caught and ignored safely without throwing runtime unhandled exceptions.

### 3. State Transition Integrity
- `FlyActionBridge` coordinates with `@rustok/cart-frontend` state: `addItem` returns a promise, opening the cart drawer on success.
- Draft mode state transitions cleanly between preview and live versions via signed Next.js draft tokens and `/api/disable-draft`.

### 4. Side-Effect Completeness
- Event listeners for `fly:action` and document `click` are cleanly unregistered in `FlyActionBridge`'s `useEffect` cleanup hook.
- `initFlyActions` uses a global document symbol (`Symbol.for("fly.actions.adapter")`) to ensure click delegation is attached exactly once per document lifecycle.

### 5. Calculation & Algorithmic Correctness
- `scopeLandingCss` implements a tokenizer respecting block nesting (`group` vs `declaration`), string quotes (`"`, `'`), comments (`/* ... */`), and identifier word boundaries (`tbody` or `.some-body` are never mistaken for `body`).
- Quantity parsing in `handleFlyAction` sanitizes string inputs to positive numbers (`Number(data.quantity) || 1`).

### 6. Order of Operations & Concurrency
- Same-origin internal link clicks are intercepted synchronously before navigation initiates, routing via Next.js client router without full page reloads.
- External links (`target="_blank"`), downloads, or explicit action triggers bypass SPA interception.

---

## 5. Verification & Testing

- **Contract Unit Tests**: `apps/next-frontend/packages/rustok-pages/src/pages-frontend.test.mjs` (8 passing unit tests).
- **Execution Command**:
  ```bash
  npm --prefix apps/next-frontend run test:pages
  ```
- **Type Checking**:
  ```bash
  npm run typecheck:next-frontend
  ```
- **Storefront Parity & Route Verification**:
  ```bash
  npm run verify:storefront:routes
  ```
