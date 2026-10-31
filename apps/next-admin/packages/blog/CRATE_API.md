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
2. **Fail-Closed Permissions:** Operations require appropriate blog permissions (`manage_posts`, `manage_categories`); unauthenticated calls fail closed.
3. **Parity with Leptos Admin:** Page capabilities, status indicators, and editorial fields maintain functional parity with `crates/modules/rustok-blog/admin`.

## Dependencies & Boundaries

- **Host Interface:** Depends on React 19 and `@/modules/registry` from `apps/next-admin`.
- **Backend API:** Communicates with `apps/server` via GraphQL endpoint (`/api/graphql`).
- **No Direct Private Access:** Never accesses database tables or server internals directly.
