# apps/next-admin/packages/blog / CRATE_API

## Purpose

`@rustok/blog-admin` (`apps/next-admin/packages/blog`) is the Next.js administration UI package for the `rustok-blog` module. It provides full editorial interfaces for blog post creation, editing, category hierarchy management, and comment moderation in the Next.js admin dashboard.

## Architecture & Responsibilities

- **Module Registration:** Registers the blog module with the host admin registry via `registerAdminModule` and supplies structured navigation items (`blogNavItems`).
- **Editorial Workspaces:** Implements pages and components for managing posts (`PostsPage`, `PostDetailPage`, `PostFormPage`, `PostForm`, `PostTable`) and categories (`CategoriesPage`, `CategoryModal`).
- **GraphQL Integration:** Directly queries and mutates the backend GraphQL API for posts, categories, and moderation state without intermediate server-side proxies.
- **Contract Integrity:** Guarded against schema drift by `scripts/verify/verify-blog-nextjs-graphql-contract.mjs`, ensuring all queried fields strictly exist in the backend Blog schema.

## Public Surface

- `blogNavItems`: Navigation configuration for the Next.js admin sidebar under `modulePlugins`.
- `PostsPage`: Article list page with search, filters, pagination, and action buttons.
- `PostDetailPage`: Article preview and detail view.
- `PostFormPage`: Article creation and editing page container.
- `PostForm`: Editorial form component with rich text content, metadata, taxonomy, and publishing state controls.
- `CategoriesPage`: Category management view with hierarchical listing.
- `CategoryModal`: Create/edit dialog for blog categories.
- `PostTable`: Data table component displaying posts with sortable columns and batch operations.
- `api/posts`: API functions for fetching, creating, updating, publishing, and deleting blog posts.
- `api/categories`: API functions for category listing and mutations.

## Invariants & Guarantees

1. **GraphQL Contract Hygiene:** All GraphQL queries, mutations, and fragments match the backend `rustok-blog` schema; no non-existent fields or invalid variable types.
2. **Fail-Closed Permissions:** Operations require appropriate blog permissions (`blog_posts:*`, `blog_categories:*`); unauthenticated calls fail closed.
3. **Editorial Provenance:** Pure Post and Category command builders keep requested/effective locale identity explicit. Fallback copy is read-only; unchanged fields are omitted and nullable changes distinguish Set from Clear. Mandatory Post revision is taken from the loaded owner snapshot.
4. **Host Locale:** Server detail loaders obtain the effective locale through `@rustok/next-fluent/server` and pass it explicitly to the GraphQL query. Module packages do not select a locale from their own headers or cookies.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19 and `@/modules/registry` from `apps/next-admin`.
- **Backend API:** Communicates with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Private Access:** Never accesses database tables or server internals directly.

## Editor verification

- `npm run test:blog` executes pure editor and actual API-mapping regressions.
- `npm run lint:blog` explicitly includes this package; the host-only `lint`
  script does not establish package lint evidence.
- `npm run typecheck` includes the package through the host TypeScript project.
- The root Blog GraphQL contract guard checks document/schema conformance.

Category loading exposes loading/error/retry states, preserves the existing
selection, and disables changes while options are unavailable. Post forms are
keyed by their owner snapshot and principal so defaults cannot leak across
editing sessions. Acknowledged mutations do not issue privileged detail reads;
navigation occurs after acknowledgement and is not a mutation-failure signal.
Schedules use offset-bearing RFC 3339 instants rather than ambiguous local
wall-clock input.

## Known Limitations / Pending Implementation

Mounted editor and accessibility parity remains unverified. Full host
TypeScript checking currently also reports pre-existing Pricing package/nav
wiring errors; editor source checking cannot certify the entire host. Local
installed ESLint may differ from the manifest version; use the declared ESLint
9 toolchain for package checks. See the Blog owner reference plan for retained
execution evidence and remaining cross-client acceptance gates.

Editor sessions are keyed by snapshot, principal, tenant and requested locale.
Unmounted sessions ignore write completions; acknowledged writes remain locked
until navigation replaces the form. This prevents duplicate creation while a
post-commit navigation is delayed. Clearing an existing canonical Post route
produces an explicit localized validation error.
