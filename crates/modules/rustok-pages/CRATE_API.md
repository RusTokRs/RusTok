# rustok-pages / CRATE_API

## Public surface

- Modules: `controllers`, `dto`, `entities`, `error`, `graphql`, `http`, `migrations`, `openapi`, `provider_health_binding`, `search_projection`, `services`, `module`.
- Types: `PagesModule`, `PageService`, `PageBuilderArtifactService`, `PageBuilderScenarioBaselineService`, `PageRouteService`, `PageRouteHistoryImportService`, `PagesProviderHealthAuthority`, `PageCacheInvalidationEventHandler`, `PageSearchProjectionSource`, `PagesError`, `PagesResult<T>`.
- Core DTOs: `CreatePageInput`, `PageBodyInput`, `PageTranslationInput`, `PublishPageInput`, `RollbackPageInput`, `SavePageDocumentInput`.

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

## Layout selection contract

`pageTemplateKeys` exposes exact-tenant, exact-locale layout keys to authenticated
`pages:create` principals. It returns only virtual `default` without `pages:manage`;
non-default keys, full records and builder composition require `pages:manage`.
Existing non-default pages may therefore be inaccessible in the builder to editors
without that grant; a page-scoped composition permission remains open. Admin clients recheck their
explicit selection immediately before creating a page, without silently substituting
`default` on errors. Both admins also opt into GraphQL `checkedLayout`: the server
accepts a single translation, requires `pages:manage` for non-default keys and checks
the exact tenant/locale layout inside the creation transaction, serialized with
same-locale catalog deletion on PostgreSQL and SQLite. PostgreSQL uses READ COMMITTED;
SQLite may return a busy error under contention. Checked non-default creation fails
closed on MySQL. Legacy create/metadata APIs still accept free-form labels and do not
participate in this guarantee; cross-locale deletion checks and legacy writes are not
serialized. This is not a revision pin: a changed or recreated layout under the same
key may be selected. Concurrency and database behavior need integration verification. Reviewed publish resolves
non-default keys for every published body locale and fails when a definition is missing.

## Starter catalog contract

`PageStarterService` and the `pageStarters`, `pageStarter`, `savePageStarter`,
`updatePageStarterMetadata`, `instantiatePageStarter`, and `deletePageStarter` GraphQL operations expose a
tenant-local library. Listing merges built-ins and tenant records by key (tenant
wins), orders by key, and filters category case-insensitively. Catalog access
requires an authenticated tenant-wide Pages grant; writes require `pages:manage`.
Saving checks key and default layout key syntax and whether Fly can clone the
document; instantiation reissues component identities. This is not full
render/readiness validation, and layout existence is not resolved without a locale.

Tenant rows carry an opaque UUID revision; built-ins have a null revision. A
save with no expected revision creates a tenant row (including an override of a
built-in); update and delete require the last observed token. Stale, missing,
or concurrently replaced rows fail with `PAGE_STARTER_REVISION_CONFLICT`.
`updatePageStarterMetadata` requires a non-null observed revision and updates only
name, category, description and suggested layout key; document and thumbnail are
untouched. It cannot create or override a built-in. Layout keys are syntax-checked,
not resolved for a locale. Deleting an override reveals the built-in. `instantiatePageStarter` also compares
`expectedRevision`: null selects a built-in and a UUID selects that exact tenant
revision. Instantiation requires the intended page locale. An unlinked starter's
embedded symbol catalog and revision are discarded; a linked starter receives the
current exact-locale site catalog and token, and unresolved references fail closed.
Because this returns the full authoring catalog, linked starters additionally require
`pages:manage`. If the catalog changes before page creation, the page save CAS
rejects the stale document. `instantiate_starter` is only a raw clone helper, not a
safe substitute for the locale-aware selection flow. The built-ins include the six formerly local admin presets; title input
is page metadata and does not rewrite the preset's example content. The additive
migration backfills existing revisions from row IDs; stop old writers while
migrating and deploying, since old binaries cannot honor the CAS contract.

`savePageStarterFromPage` creates a new tenant key from the persisted exact-locale
working body (draft if present, otherwise current), requiring `pages:manage` and
source access. It rejects built-in keys, existing keys, missing or non-builder
bodies, and strips the site's embedded symbol definitions and revision while
retaining references. It returns the captured locale, state and body revision;
no unsaved editor state is included. Source reads and starter insertion are not
revision-locked in one transaction, so the returned revision describes the captured
body, not a guarantee that it remained the latest through insertion. The suggested
layout is copied by key and is not checked for locale availability.

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
- `publish_reviewed` decides everything on data read under its transaction's
  locks, in the order recorded in
  `rustok-page-builder/contracts/page-builder-publish-runtime-review.json`
  (`new_operation_order`). The scenario gate and the compilation may be computed
  before the transaction; the transaction uses that result only when the
  expanded documents and the promoted baseline it read under its locks are the
  ones the result was computed from, and otherwise runs both steps itself.
- Missing providers or invalid projects fail visibly; no fallback model is
  synthesized. Starter instantiation propagates Fly's ambiguous-identity remap failure as a
  Pages validation error.

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
