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
- `scope_landing_css`: Utilities wrapping project CSS in `@scope` rules and remapping root selectors (`body`, `html`) to `:scope`.
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
- **CSS Isolation & Containment**: User-generated styles are strictly scoped via `@scope` with `isolation: isolate; contain: style layout;` preventing host shell contamination.
- **HTML Sanitization**: All rendered markup passes through Fly's strict sanitizer, stripping raw scripts, invalid URLs, and unauthorized event handlers.
- **Deterministic Action Forwarding**: Storefront action buttons emit standard custom events without bypassing platform cart or navigation pipelines.

### Events / Outbox Side Effects
- Storefront rendering is read-only and does not emit transactional outbox events.
- Interactive user actions are bridged to client-side DOM events (`rustok:cart:add`, in-page anchors).

### Errors / Failure Codes
- `fly::FlyError`: Document parsing, validation, or sanitization failures.
- `PageBuilderStorefrontViewModel`: Captures render errors cleanly and displays accessible error alerts without panicking.
