# @rustok/pages-admin — Canonical API Specification

## 1. Package Overview

`@rustok/pages-admin` provides the module-owned Next.js administration surface for the RusToK CMS Pages module. Following the Fluid Frontend Architecture (FFA) boundary contract, both the Rust Leptos admin and Next.js admin share the exact same `rustok-pages` async-graphql backend schema.

This package manages page lifecycle operations (listing, creating initial drafts, unpublishing, deleting, and deep-linking into visual editors or storefront draft previews). Visual drag-and-drop page editing remains owned by `rustok-pages-admin` (the Rust FFA builder); deep links seamlessly connect operators between interfaces without duplicating editor complexity.

- **Package Name**: `@rustok/pages-admin`
- **Location**: `apps/next-admin/packages/pages`
- **Owner**: `rustok-pages` module frontend surface
- **Status**: Stable / Verified

## 2. Public API Surface

### Exported Components & Pages

- `PagesPage` (default export & alias `PagesAdminPage`): Server component rendering the pages table, status indicators, and actions.
- `PageActions`: Client component providing form inputs to create initial drafts and action buttons to unpublish or delete pages with router revalidation.

### Navigation

- `pagesNavItems: NavItem[]`: Declares `/dashboard/pages` under the `modulePlugins` group with `['p', 'l']` keyboard shortcuts.

### API & Data Fetching

- `PAGES_LIST_QUERY`: GraphQL query retrieving paginated pages with status, template, translations, channels, and timestamp.
- `CREATE_PAGE_MUTATION`: GraphQL mutation initializing a draft page with localized translations.
- `UNPUBLISH_PAGE_MUTATION`: GraphQL mutation transitioning a published page back to draft, supporting optimistic `expectedVersion` concurrency check.
- `DELETE_PAGE_MUTATION`: GraphQL mutation permanently removing a page.
- `listPages(filter, opts)`: Queries paginated pages with clamped bounds (`page >= 1`, `1 <= perPage <= 100`).
- `createPage(input, opts)`: Creates a draft page; validates non-empty title/locale and enforces `publish: false`.
- `unpublishPage(id, expectedVersion, opts)`: Unpublishes an active page.
- `deletePage(id, opts)`: Deletes a page by UUID.
- `builderUrlForPage(builderBaseUrl, pageId)`: Generates deep link to the Rust FFA visual page builder.
- `storefrontUrlForPage(storefrontBaseUrl, slug, locale, status, previewSecret)`: Generates canonical storefront URLs for published pages or secure draft preview links with secret tokens.

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
  - `storefrontUrlForPage` canonical routes, draft preview tokens, and leading slash normalization.
  - GraphQL schema query and mutation field validation.
  - `listPages` pagination clamping and tenant context propagation.
  - `createPage` title/locale validation and `publish: false` invariant enforcement.
  - `unpublishPage` and `deletePage` ID validation and OCC parameter forwarding.
