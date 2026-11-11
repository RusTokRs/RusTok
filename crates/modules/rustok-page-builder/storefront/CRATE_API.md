# Crate API: rustok-page-builder-storefront

Canonical public contract and domain architecture specification for `rustok-page-builder-storefront`.

---

## Public surface

### Storefront Components
- `PageBuilderStorefront`: Dynamic SSR/hydrate component rendering Page Builder project documents with scoped CSS and Fly actions.
- `StaticLandingStorefront`: Direct SSR renderer for pre-compiled static landing artifact HTML and scoped CSS without `<iframe>`.
- `PageBuilderView`: Routable storefront page view component with localized metadata and error fallbacks.

### Core Rendering Primitives
- `render_storefront_page`: Renders project data into a `StorefrontPageOutput` with default policy.
- `render_storefront_page_with_rich_text`: Renders project data with optional platform rich-text renderer.
- `render_storefront_page_with_context`: Renders project data with dynamic runtime context for repeaters/conditions.
- `scope_landing_css(raw_css, scope_selector)`: Returns the stylesheet as one complete `@scope (<scope_selector>)` block with the document-level selectors (`body`, `html`, `:root`) remapped to `:scope` where they appear in selector position. The stylesheet is tokenized by the CSS Syntax rules; a `}` that would close the wrapper early is dropped, and unclosed blocks, strings, comments and `url()` tokens are closed. Only the function's own output for the same selector passes through unchanged. The result never contains `</style` (written `<\/style`). Empty or whitespace-only input yields `""`.
- `LANDING_SCOPE_SELECTOR`: The scope root every surface of this crate uses — `[data-fly-page-body]`, the element each surface renders around the landing markup.
- `SharedRichTextRenderer`: Thread-safe shared handle to host rich-text renderer.

### Output Models
- `StorefrontPageOutput`: Contains rendered HTML, scoped CSS, and `PageHead` metadata.
- `StorefrontRuntimePageOutput`: Contains runtime render results including diagnostics and repetition counters.
- `PageBuilderStorefrontViewModel`: Framework-agnostic view-model for page rendering states and titles.

### Browser Bridges & Assets
- `FLY_STOREFRONT_BRIDGE_JS`: JavaScript bridge forwarding `fly:action` custom events to RusToK commerce events.
- `fly_browser`: Re-export of Fly browser utilities and interactive action scripts.

---

## Platform Capability Bindings

- **`fly`**: Core layout engine, document decoder, and sanitized HTML generation.
- **`fly-browser`**: Client-side action execution runtime (`FLY_ACTIONS_JS`).
- **`rustok-ui-core`**: Core UI traits, normalized view-models, and design token integration.
- **`rustok-ui-i18n`**: Localized Fluent catalog integration for storefront error and fallback labels.
- **`rustok-page-builder`**: Core domain logic, runtime context preflight, and static compilation policies.

---

## Domain Collaborations

- **`rustok-page-builder`**: Consumes core document representations, static landing compilation, and runtime render pipelines.
- **`rustok-pages`**: Serves as the primary content source for published page builder pages.
- **`rustok-cart`**: Target for shopping cart actions bridged via `FLY_STOREFRONT_BRIDGE_JS`.

---

## Minimum Contract Set

### Input DTOs/Commands
- `PageSelection`: Selection criteria for specific sub-pages in a multi-page document (`First`, `ById`, `BySlug`).
- `RenderPolicy`: Sanitization and rendering behavior switches (component instrumentation, strict CSS filters).
- `PageBuilderStorefrontViewModel`: Pure Rust view-model driving storefront page presentation.

### Domain Invariants
- **Fluid Frontend Architecture (FFA)**: Strict separation between pure Rust view-models in `core.rs` and Leptos UI rendering components in `ui.rs` and `lib.rs`.
- **CSS Isolation & Containment**: Every surface that emits project CSS scopes it with `scope_landing_css` to `LANDING_SCOPE_SELECTOR`; no surface emits it unscoped, and the scope root does not depend on the host-supplied `class`. At-rules that define document-wide names (`@keyframes`, `@font-face`, `@property`) are not confined by `@scope`.
- **Storefront parity**: `scope_landing_css` and `scopeLandingCss` (`apps/next-frontend/packages/rustok-pages/src/components/css-scope.ts`) return the same text for the same input and selector. `contracts/landing-css-scope-parity.json` holds the shared vectors; both test suites assert it. The Next.js storefront also renders the vectors in a browser (`apps/next-frontend/tests/e2e/landing-css-scope.spec.ts`); that check reaches this crate only through the parity of the two functions.
- **HTML Sanitization**: All rendered markup passes through Fly's strict sanitizer, stripping raw scripts, invalid URLs, and unauthorized event handlers.
- **Deterministic Action Forwarding**: Storefront action buttons emit standard custom events without bypassing platform cart or navigation pipelines.

### Events / Outbox Side Effects
- Storefront rendering is read-only and does not emit transactional outbox events.
- Interactive user actions are bridged to client-side DOM events (`rustok:cart:add`, in-page anchors).

### Errors / Failure Codes
- `fly::FlyError`: Document parsing, validation, or sanitization failures.
- `PageBuilderStorefrontViewModel`: Captures render errors cleanly and displays accessible error alerts without panicking.
