# @rustok/pages-admin — Canonical API Specification

## 1. Package Overview

`@rustok/pages-admin` provides the module-owned Next.js administration surface for the RusToK CMS Pages module. Following the Fluid Frontend Architecture (FFA) boundary contract, both the Rust Leptos admin and Next.js admin share the exact same `rustok-pages` async-graphql backend schema.

This package manages page lifecycle operations (listing, creating initial drafts, unpublishing, deleting, and deep-linking into visual editors or storefront draft previews). Visual drag-and-drop page editing remains owned by `rustok-pages-admin` (the Rust FFA builder); deep links seamlessly connect operators between interfaces without duplicating editor complexity.

- **Package Name**: `@rustok/pages-admin`
- **Location**: `apps/next-admin/packages/pages`
- **Owner**: `rustok-pages` module frontend surface
- **Status**: In progress; starter integration requires scoped verification.

## 2. Public API Surface

### Exported Components & Pages

- `PagesPage` (default export & alias `PagesAdminPage`): Server component rendering the pages table, status indicators, and actions.
- `PageActions`: Client component providing form inputs to create initial drafts and action buttons to unpublish or delete pages with router revalidation.

### Navigation

- `pagesNavItems: NavItem[]`: Declares `/dashboard/pages` under the `modulePlugins` group with `['p', 'l']` keyboard shortcuts.

### API & Data Fetching

- `PAGES_LIST_QUERY`: GraphQL query retrieving paginated pages with status, template, translations, channels, and timestamp.
- `PAGE_STARTERS_QUERY`, `listPageStarters`: Read the tenant starter catalog.
- `PAGE_LAYOUT_KEYS_QUERY`, `listPageLayoutKeys`: Read usable exact-locale keys using `pages:create`; only `default` is available without `pages:manage`, which non-default builder composition currently requires. The create flow requires an explicit available selection, rechecks it and requests `checkedLayout` on create. Server-side selection is serialized against same-locale deletion on PostgreSQL/SQLite; non-default selection fails closed on MySQL. No layout revision is pinned.
- `INSTANTIATE_PAGE_STARTER_MUTATION`, `instantiatePageStarter`: Clone the selected revision for the target locale and require an object document. Linked site-symbol starters require `pages:manage` and available exact-locale symbols.
- `SAVE_STARTER_FROM_PAGE_MUTATION`, `saveStarterFromPage`: create a new key from an exact-locale saved working body and return its revision and state. The UI does not capture unsaved builder state; the host hides the action without an effective `pages:manage` grant.
- `UPDATE_STARTER_METADATA_MUTATION`, `updateStarterMetadata`, `DELETE_STARTER_MUTATION`, `deleteStarter`: manage only observed tenant revisions; metadata updates preserve document and thumbnail, and deleting an override reveals its built-in. Built-ins are read-only in the UI.
- `CREATE_PAGE_MUTATION`: GraphQL mutation initializing a draft page with localized translations and an optional Fly document.
- `UNPUBLISH_PAGE_MUTATION`: GraphQL mutation transitioning a published page back to draft, supporting optimistic `expectedVersion` concurrency check.
- `DELETE_PAGE_MUTATION`: GraphQL mutation permanently removing a page.
- `listPages(filter, opts)`: Queries paginated pages with clamped bounds (`page >= 1`, `1 <= perPage <= 100`).
- `createPage(input, opts)`: Creates a draft page; validates non-empty title/locale and enforces `publish: false`.
- `unpublishPage(id, expectedVersion, opts)`: Unpublishes an active page.
- `deletePage(id, opts)`: Deletes a page by UUID.
- `builderUrlForPage(builderBaseUrl, pageId)`: Generates deep link to the Rust FFA visual page builder.
- `storefrontUrlForPage(storefrontBaseUrl, slug, locale, status)`: Canonical storefront URL of a published page; `null` for a page that is not published, because the storefront renders the published landing only.

## 3. Dependencies & Architecture Boundaries

- **Direct Dependencies**: `react: ^19.2.0`, `next: ^15.0.0` (peer / host).
- **Host Integration**: Registered in Next.js admin registry via `registerAdminModule({ id: 'pages', name: 'Pages', navItems: pagesNavItems })`.
- **Backend Boundary**: Communicates with the `rustok-pages` async-graphql schema via `graphqlRequest`.

## 4. Invariants & Security

- **Strict Validation & Fail-Closed Guards**: `createPage` rejects empty or whitespace-only titles and locales before initiating network requests. `unpublishPage` and `deletePage` reject empty page IDs.
- **Reviewed Publishing Enforcement**: `createPage` explicitly sets `publish: false` in its payload, preventing accidental unreviewed publishing from the list surface.
- **Optimistic Concurrency Control**: `unpublishPage` accepts `expectedVersion` to prevent stale overwrites.
- **URL Parameter Sanitization**: `builderUrlForPage` and `storefrontUrlForPage` strip trailing and leading slashes to prevent malformed or double-slash paths, and encode all query parameters.
- **Error Isolation**: Page load failures are caught and rendered via accessible alert banners (`role="alert"`), preventing full tree unmounts.

## 5. Verification & Testing

- **Contract Tests**: `packages/pages/src/pages-admin.test.mjs` executed via `npm --prefix apps/next-admin run test:pages`.
- **Verified Coverage**:
  - `pagesNavItems` routing and keyboard shortcuts (`p, l`).
  - `builderUrlForPage` deep-link generation and trailing slash handling.
  - `storefrontUrlForPage` canonical routes, `null` for unpublished pages, and leading slash normalization.
  - GraphQL schema query and mutation field validation.
  - `listPages` pagination clamping and tenant context propagation.
  - `createPage` title/locale validation and `publish: false` invariant enforcement.
  - `unpublishPage` and `deletePage` ID validation and OCC parameter forwarding.
