# `rustok-pages-storefront` Public API & Architecture Contract

## Minimum Contract Set

- **Module Authority**: `rustok-pages-storefront` is the authoritative Leptos storefront UI package for static CMS pages, legal pages, about pages, and published Page Builder landing pages.
- **Symmetric FFA Boundary**: Framework-agnostic view-models and string formatting logic live strictly in `src/core.rs` with zero UI framework imports and 100% pure unit test coverage. Presentation and event handling live strictly in `src/ui/leptos.rs`.
- **Dual-Path Data Access**: Supports both native server functions (`#[server]`) for monolith/hydrate deployments and headless GraphQL queries for decoupled storefront architectures.
- **Seamless Page Builder Delivery**: Renders published landing pages directly using server-side HTML and scoped CSS via `PageBuilderPageBody` and `StaticLandingStorefront`, eliminating `<iframe>` wrappers on storefront views while retaining full interactive action support.
- **Storefront Route Resolution**: Resolves canonical slug routing, redirects, and locale fallbacks via `resolve_storefront_page_route` and `StorefrontPageRouteDecision`.

## Platform Capability Bindings

- **`fly` & `rustok-page-builder-storefront`**: `StaticLandingStorefront`, `PageBuilderStorefront`, `SharedRichTextRenderer`, and `FLY_ACTIONS_JS` runtime bridge.
- **`rustok-ui-core`**: `UiRouteContext`, `parse_ui_csv`, `normalize_ui_text`, and status badges.
- **`rustok-ui-transport`**: `execute_selected_transport`, `UiTransportPath`, and `UiTransportError`.
- **`leptos-ui-routing`**: URL query parameter integration and route state synchronization.

## Domain Collaborations

- **`rustok-pages`**: Consumes published page detail, translations, and body revisions from the backend CMS service.
- **`rustok-channel`**: Respects multi-channel scoping and channel-specific page visibility.
- **`rustok-tenant`**: Multi-tenant resolution via host request context.

## Public surface

### Components
- `PagesView`: Primary Leptos storefront root view component rendering published CMS pages and page lists.
- `PageBuilderPageBody`: High-level renderer for page builder document formats (`PAGE_BUILDER_DOCUMENT_FORMAT`, `STATIC_LANDING_PAYLOAD_BODY_FORMAT`, `STATIC_LANDING_URL_BODY_FORMAT`).

### Route Resolution APIs
- `resolve_storefront_page_route`: Asynchronously resolves page slug routing decisions for storefront navigation.
- `StorefrontPageRouteDecision`: Encapsulates route disposition (`Canonical`, `Redirect`, `Gone`, `NotFound`, `Conflict`), canonical slug, page ID, and generation markers.
- `StorefrontPageRouteDisposition`: Enumeration of route outcomes (`Canonical`, `Redirect`, `Gone`, `NotFound`, `Conflict`).

### Document & Format Constants & Functions
- `PAGE_BUILDER_DOCUMENT_FORMAT`: Format string identifier (`"grapesjs"`).
- `STATIC_LANDING_PAYLOAD_BODY_FORMAT`: Format string identifier (`"fly_artifact_payload"`).
- `STATIC_LANDING_URL_BODY_FORMAT`: Format string identifier (`"fly_artifact_url"`).
- `is_page_builder_body`: Checks whether a given `PageBody` is recognized as a page builder document.
- `decode_page_builder_body`: Decodes a JSON page builder document into `serde_json::Value`.
- `decode_static_landing_payload`: Decodes static landing payload into `StaticLandingBodyPayload`.

### Inline Edit APIs (feature = "inline-edit")
- `PagesAuthenticatedInlineEditSurface`: Component rendering the authenticated inline editing toolbar on storefront pages.
- `PagesInlineEditBootstrap`: DTO containing the edit token, session claims, and authoring capabilities.
- `fetch_pages_inline_edit_bootstrap`: Loads inline-edit bootstrap credentials.
- `commit_pages_inline_edit`: Commits an inline-edited document back to the pages service.

### Input DTOs/Commands

- `PageDetail`: Storefront page entity containing translation, body, and effective locale.
- `PageListItem`: Summary item for page directory listings (`id`, `title`, `slug`, `status`, `template`).
- `PageBody`: Content payload (`locale`, `content`, `format`).
- `StaticLandingBodyPayload`: Immutable published landing payload with pre-compiled HTML, CSS, and optional artifact URL.
- `StorefrontPagesData`: Container for page list and currently selected page detail.

### Domain Invariants

- **Strict FFA Separation**: `src/core.rs` contains zero imports of UI framework types (`leptos::*`, `dioxus::*`, etc.). All computed labels, empty states, and title resolution must be verifiable without a DOM or reactive runtime.
- **Native Render Priority**: When `STATIC_LANDING_PAYLOAD_BODY_FORMAT` is present, pages MUST render inline via `StaticLandingStorefront` and `@scope` CSS, never wrapping inside an `<iframe>`.
- **Sandbox Fallback Invariant**: When fallback to external URL iframe is required (`STATIC_LANDING_URL_BODY_FORMAT`), the `<iframe>` must strictly declare `sandbox="allow-forms allow-same-origin"`.
- **Locale Fallback Order**: When requested locale content is missing, the resolver falls back to default tenant locale while preserving `effective_locale` annotations.

### Events / Outbox Side Effects

- **Storefront Read Path Invariance**: Browsing and rendering published pages produces strictly zero outbox messages or state mutations.
- **Inline Editing Mutations**: When inline editing is enabled and committed, draft changes are dispatched through the authenticated `PageService` GraphQL/ServerFn mutation endpoints, leaving outbox event generation to the backend host.

### Errors / Failure Codes

- `ApiError::Graphql(String)`: Headless GraphQL query transport failure.
- `ApiError::ServerFn(String)`: Native server function transport failure.
- `PagesTransportError`: Unified UI transport error wrapper (`UiTransportError`).
- `StorefrontPageRouteDisposition::NotFound`: HTTP 404 — No published page matches the requested slug.
- `StorefrontPageRouteDisposition::Redirect`: HTTP 301/308 — Slug has been redirected to canonical path.
- `StorefrontPageRouteDisposition::Gone`: HTTP 410 — Page has been deleted or permanently archived.
- `StorefrontPageRouteDisposition::Conflict`: HTTP 409 — Route collision across channels or languages.
