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
- `StorefrontPageOutput`: Wraps Fly's rendered page HTML, CSS and `PageHead` metadata. The Leptos surface scopes the CSS when it renders; the core output is not itself a scoped stylesheet.
- `StorefrontRuntimePageOutput`: Contains runtime render results including diagnostics and repetition counters.
- `PageBuilderStorefrontViewModel`: Framework-agnostic view-model for page rendering states and titles.

### Browser Bridges & Assets
- `FLY_STOREFRONT_BRIDGE_JS`: JavaScript bridge for landing-local navigation, validated Product buyer handoff and acknowledged Cart open/close/toggle requests. Same-context fragment links and `navigate` events resolve only inside the nearest landing; missing, empty and malformed fragments do not trigger host navigation. Scrolling honors reduced motion. See the [README](README.md#purchase-and-cart-bridge) for the shared payload and host event protocol.
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
- **`rustok-product` / `rustok-cart`**: Product validates and quotes a handoff; Cart owns cart actions and observations. The Leptos host now composes a shared event consumer in source, pending mounted and real-service verification.

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
- **HTML ownership**: Document rendering uses Fly's sanitizer. `StaticLandingStorefront` trusts the pre-compiled published artifact passed by the host and performs no second sanitization of its `html` prop; arbitrary untrusted HTML must not be supplied there.
- **Action ownership**: Only `emit_event` host events are interpreted; provider actions remain provider-owned. The shared `buyerHref` points to `/[locale]/products/[handle]`; Leptos translates it to `/[locale]/modules/products?handle=…`. SKU and quantity are validated, conflicting selectors are rejected and Product owns the quote and purchase confirmation.
- **CSP**: All four surfaces read `UiCspNonce` for their trusted inline style and scripts. `apps/storefront` does not revalidate/cache a nonce-bearing canonical Pages composition. Neither this nor the synthetic CSP test grants a nonce to tenant-authored markup.

### Events / Outbox Side Effects
- Storefront rendering is read-only and does not emit transactional outbox events.
- Cart controls dispatch cancelable `rustok:cart:{open,close,toggle}` requests. The Cart consumer acknowledges a handled request with `preventDefault()`; without it, the bridge reports unavailable. `rustok:cart:updated` updates counters and `{status: "unknown"}` invalidates them; `rustok:cart:request-count` asks a mounted owner to republish after a late bridge installation. The purchase handoff is navigation, not a Cart add event.

### Errors / Failure Codes
- `fly::FlyError`: Document parsing, validation, or sanitization failures.
- `PageBuilderStorefrontViewModel`: Captures render errors cleanly and displays accessible error alerts without panicking.
