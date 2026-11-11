# `rustok-pages-storefront` Public API & Architecture Contract

## Minimum Contract Set

- **Module Authority**: `rustok-pages-storefront` is the authoritative Leptos storefront UI package for static CMS pages, legal pages, about pages, and published Page Builder landing pages.
- **Symmetric FFA Boundary**: Framework-agnostic view-models and string formatting logic live strictly in `src/core.rs` with zero UI framework imports and 100% pure unit test coverage. Presentation and event handling live strictly in `src/ui/leptos.rs`.
- **Dual-Path Data Access**: Supports both native server functions (`#[server]`) for monolith/hydrate deployments and headless GraphQL queries for decoupled storefront architectures.
- **Published Landing Delivery**: Renders the published landing artifact of a page (sanitized markup and stylesheet stored by the reviewed publish) inline via `PublishedLandingBody` and `StaticLandingStorefront`, without `<iframe>` wrappers, while retaining full interactive action support.
- **No Authoring Document on the Storefront**: The page body (the Page Builder authoring document) is never requested, transported, or rendered by this package. Both transports carry only `PublishedLanding`.
- **Storefront Route Resolution**: Resolves canonical slug routing, redirects, and locale fallbacks via `resolve_storefront_page_route` and `StorefrontPageRouteDecision`.

## Platform Capability Bindings

- **`rustok-page-builder-storefront`**: `StaticLandingStorefront` (scoped stylesheet, `FLY_ACTIONS_JS` runtime bridge) for published landings; `PageBuilderAuthenticatedInlineStorefront` for the `inline-edit` surface.
- **`rustok-ui-core`**: `UiRouteContext`, `parse_ui_csv`, `normalize_ui_text`, and status badges.
- **`rustok-ui-transport`**: `execute_selected_transport`, `UiTransportPath`, and `UiTransportError`.
- **`leptos-ui-routing`**: URL query parameter integration and route state synchronization.

## Domain Collaborations

- **`rustok-pages`**: Consumes published page detail, translations, and the published landing artifact from the backend CMS service: `PageBuilderArtifactService::load_public_bound_artifact_with_fallback` on the native path, `pageBySlug { landing { ... } }` on the GraphQL path.
- **`rustok-channel`**: Respects multi-channel scoping and channel-specific page visibility.
- **`rustok-tenant`**: Multi-tenant resolution via host request context.

## Public surface

### Components
- `PagesView`: Primary Leptos storefront root view component rendering published CMS pages and page lists.
- `PublishedLandingBody`: Renders one `PublishedLanding` inline through `StaticLandingStorefront`.

### Route Resolution APIs
- `resolve_storefront_page_route`: Asynchronously resolves page slug routing decisions for storefront navigation.
- `StorefrontPageRouteDecision`: Encapsulates route disposition (`Canonical`, `Redirect`, `Gone`, `NotFound`, `Conflict`), canonical slug, page ID, and generation markers.
- `StorefrontPageRouteDisposition`: Enumeration of route outcomes (`Canonical`, `Redirect`, `Gone`, `NotFound`, `Conflict`).

### Published Landing
- `PublishedLanding`: The published landing artifact of one page locale (`locale`, `html`, `css`, `artifact_hash`; `artifactHash` on the wire). `html` is body markup and `css` is the unscoped stylesheet, exactly as stored by the reviewed publish.

### Inline Edit APIs (feature = "inline-edit")
- `PagesAuthenticatedInlineEditSurface`: Component rendering the authenticated inline editing toolbar on storefront pages.
- `PagesInlineEditBootstrap`: DTO containing the edit token, session claims, and authoring capabilities.
- `fetch_pages_inline_edit_bootstrap`: Loads inline-edit bootstrap credentials.
- `commit_pages_inline_edit`: Commits an inline-edited document back to the pages service.

### Input DTOs/Commands

- `PageDetail`: Storefront page entity containing translation, effective locale, and `landing: Option<PublishedLanding>`. Unknown fields are rejected on decode, so a cached snapshot of another shape is a cache miss, not a page without content.
- `PageListItem`: Summary item for page directory listings (`id`, `title`, `slug`, `status`, `template`).
- `StorefrontPagesData`: Container for page list and currently selected page detail.

### Domain Invariants

- **Strict FFA Separation**: `src/core.rs` contains zero imports of UI framework types (`leptos::*`, `dioxus::*`, etc.). All computed labels, empty states, and title resolution must be verifiable without a DOM or reactive runtime.
- **Published Landing Is the Only Content Source**: A selected page renders its `PublishedLanding` inline via `StaticLandingStorefront` with a stylesheet scoped to the landing element, never inside an `<iframe>`. A page without a published landing renders the empty state (`pages.body.empty`); there is no fallback to the page body or to any other markup.
- **Artifact Owner Decides**: Page status, channel visibility, the locale binding, and artifact integrity are decided by `rustok-pages` when the landing is loaded. This package does not re-derive markup from the full artifact document; an integrity failure is a load error.
- **Same Shape on Both Transports**: The native server function and the GraphQL adapter produce the same `PageDetail`/`PublishedLanding` shape.
- **Locale Fallback Order**: When requested locale content is missing, the resolver falls back to default tenant locale while preserving `effective_locale` annotations.

### Events / Outbox Side Effects

- **Storefront Read Path Invariance**: Browsing and rendering published pages produces strictly zero outbox messages or state mutations.
- **Inline Editing Mutations**: When inline editing is enabled and committed, draft changes are dispatched through the authenticated `PageService` GraphQL/ServerFn mutation endpoints, leaving outbox event generation to the backend host.

### Errors / Failure Codes

- `ApiError::Graphql(String)`: Headless GraphQL query transport failure.
- `ApiError::ServerFn(String)`: Native server function transport failure.
- `PagesTransportError`: Unified UI transport error wrapper (`UiTransportError`).
- `StorefrontPageRouteDisposition::NotFound`: HTTP 404 — No published page with a published landing matches the requested slug on the request channel.
- `StorefrontPageRouteDisposition::Redirect`: HTTP 301/308 — Slug has been redirected to canonical path.
- `StorefrontPageRouteDisposition::Gone`: HTTP 410 — Page has been deleted or permanently archived.
- `StorefrontPageRouteDisposition::Conflict`: HTTP 409 — Route collision across channels or languages.
