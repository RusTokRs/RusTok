# rustok-pages / CRATE_API

## Public modules

`controllers`, `dto`, `entities`, `error`, `graphql`, `http`, `migrations`, `openapi`, `provider_health_binding`, `search_projection`, `services`, `module`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `PagesModule` implementing `RusToKModule` (slug `"pages"`) and `MigrationSource`.
- **Page Services**:
  - `PageService`: Manages page creation, localization, template bindings, inline editing, draft updates, and status transitions.
  - `PageBuilderArtifactService`: Compiles and audits deterministic landing artifacts for storefront rendering.
  - `PageBuilderScenarioBaselineService`: Manages scenario baselines for visual regression testing and release readiness.
  - `PageRouteService` & `PageRouteHistoryImportService`: Handles canonical page routes, aliases, and redirects.
  - `MenuService`: Exact-locale navigation menus and item hierarchies.
- **Cache & Search Integration**:
  - `PageCacheInvalidationEventHandler`: Subscribes to page lifecycle events to invalidate cached storefront landing projections.
  - `PageSearchProjectionSource`: Feeds search indexing with localized page content.
- **Provider Health Authority**:
  - `PagesProviderHealthAuthority`: Verifies visual builder provider health and deployment identity.

## Domain Collaborations

- **`rustok-page-builder`**: Core engine for parsing and compiling page builder documents.
- **`rustok-content`**: Shares locale resolution and canonical route formatting.
- **`rustok-outbox` / `rustok-events`**: Emits transactional events upon page lifecycle mutations.
- **`rustok-seo-targets`**: Registers page entity metadata as an SEO target.

## Primary public types

- `PagesModule`
- `PageService`
- `PageBuilderArtifactService`
- `PageBuilderScenarioBaselineService`
- `MenuService`
- `Page`, `Menu`, published artifact entities
- `PagesError`, `PagesResult<T>`

## Current document contract

- `PageBodyInput` accepts a localized Page Builder JSON value through its sole
  `document` payload field. It has no caller-selected format and no parallel
  serialized-content field; Pages persists the canonical internal `grapesjs`
  invariant.
- `pages[].component` is the component-tree authority.
- Page metadata, channel visibility and the Fly document are Pages-owned data.
- Published storefront output is selected through immutable landing artifacts.
- There is no public block entity, block service, block DTO or block mutation.

## Current menu contract

- Base `menus` and `menu_items` rows contain only language-neutral identity,
  hierarchy, routing and presentation mechanics.
- Menu names and item titles exist only in tenant-composite translation rows.
- The host passes an already-resolved effective locale to `MenuService`; the
  service requires exact menu and item translations for that locale.
- Missing localized navigation fails visibly. English and arbitrary first-row
  fallback selection are not part of the Pages menu runtime.
- Every item created through the service carries exactly the locale set owned by
  its parent menu.
- GraphQL exposes `menu(id, locale)` and `createMenu(input, locale)` using the
  same exact-locale DTO contract.
- HTTP exposes `GET /api/menus/{id}` and `POST /api/admin/menus`; request locale
  defaults to the host-resolved `RequestContext.locale`.
- Menu location is descriptive until a dedicated active-location identity is
  enforced. Adapters must not choose an arbitrary first menu by location.

## Events

Page lifecycle events are published through `TransactionalEventBus` in the same
transaction as the domain mutation.

## Domain invariants

- Tenant/resource isolation and effective permission checks are mandatory.
- Slugs are unique per tenant and locale.
- Writes use optimistic page versions.
- Builder feature gates fail with typed errors.
- Metadata-only changes must not replace an existing page body.
- Publishing a Fly document validates readiness, compiles a deterministic
  artifact and binds it before the published state becomes visible.
- Missing providers or invalid projects fail visibly; no fallback model is
  synthesized.

## Adapter obligations

Changes to public DTO fields require synchronized GraphQL, HTTP, admin and
storefront updates. Error classes for validation, authorization, conflict,
feature-disabled, integrity and not-found outcomes must retain their semantics.

## Minimum Contract Set

### Input DTOs/Commands

- `CreatePageInput`, `UpdatePageInput`, `PageTranslationInput`, `PageBodyInput`: Control core page attributes and visual document bindings.
- `CreateMenuInput`, `UpdateMenuInput`, `MenuItemInput`: Manage localized menu hierarchy.
- `ImportPageRouteHistoryInput`, `AuditPageArtifactsInput`: Manage routing history and artifact integrity.

### Domain Invariants

- Tenant isolation and permission enforcement are non-negotiable across all mutations.
- Unique slug constraints per tenant and locale prevent routing collisions.
- Published state requires pre-compiled, validated deterministic landing artifacts.

### Events / Outbox Side Effects

- Page creations, updates, publishings, rollbacks, and deletions publish domain events via `TransactionalEventBus`.
- Event payloads adhere to platform-wide JSON schema invariants.

### Errors / Failure Codes

- `PagesError` variants: `Validation`, `NotFound`, `Forbidden`, `Conflict`, `Database`, `Internal`.
- Typed error codes (e.g. `CANNOT_DELETE_PUBLISHED_ERROR_CODE`) are mapped to consistent client status codes.
