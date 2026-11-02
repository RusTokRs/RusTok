# @rustok/media-admin — Canonical API Specification

## 1. Package Overview

`@rustok/media-admin` provides the module-owned Next.js admin interface for the RusToK media library. It connects to the `rustok-media` backend GraphQL API to list, inspect, and delete tenant assets. Asset uploads remain delegated to domain-specific upload flows (such as blog post header attachments or product catalog media), while this surface serves as the central operational management view.

- **Package Name**: `@rustok/media-admin`
- **Location**: `apps/next-admin/packages/media`
- **Owner**: `rustok-media` module frontend surface
- **Status**: Stable / Verified

## 2. Public API Surface

### Exported Components & Pages

- `MediaPage` (default export & alias `MediaAdminPage`): Server component rendering the media library table, usage statistics (`fileCount`, `totalBytes`), preview links, MIME types, and action controls.
- `MediaActions`: Client component providing interactive delete confirmation with optimistic transition state and router refresh.

### Navigation

- `mediaNavItems: NavItem[]`: Declares the `/dashboard/media` route under the `modulePlugins` navigation group.

### API & Data Fetching

- `MEDIA_LIST_QUERY`: GraphQL query retrieving paginated media items (`items { id, tenantId, ownerModule, filename, originalName, mimeType, size, publicUrl, width, height, createdAt }`, `total`).
- `MEDIA_USAGE_QUERY`: GraphQL query returning aggregated tenant media metrics (`fileCount`, `totalBytes`).
- `DELETE_MEDIA_MUTATION`: GraphQL mutation executing asset deletion by tenant ID and asset UUID.
- `listMedia(opts, pagination)`: Fetches paginated media items with clamped bounds (`page >= 1`, `1 <= limit <= 100`).
- `mediaUsage(opts)`: Retrieves asset storage metrics for the tenant.
- `deleteMedia(id, opts)`: Deletes a specific media asset permanently.
- `formatBytes(bytes: number): string`: Formats raw bytes into human-readable strings (`B`, `KB`, `MB`, `GB`) with lower-bound clamping for negative values.

## 3. Dependencies & Architecture Boundaries

- **Direct Dependencies**: `react: ^19.2.0`, `next: ^15.0.0` (peer / host).
- **Host Integration**: Registered in Next.js admin via `registerAdminModule({ id: 'media', name: 'Media', navItems: mediaNavItems })`.
- **Backend Boundary**: Communicates with the `rustok-media` async-graphql backend schema via `graphqlRequest`.

## 4. Invariants & Security

- **Strict Tenant Scoping**: All mutations and queries require an active `tenantId` in `opts`. Calling `listMedia`, `mediaUsage`, or `deleteMedia` without `opts.tenantId` fails closed by throwing an Error before initiating any network request.
- **Bounds Checking**: Byte formatting handles negative values safely by returning `0 B`. Pagination clamps `page` to a minimum of 1 and `perPage` to between 1 and 100.
- **Error Isolation**: Server rendering in `MediaPage` wraps data loading in `try/catch`, rendering an accessible alert banner (`role="alert"`) rather than crashing the page tree.

## 5. Verification & Testing

- **Contract Tests**: `packages/media/src/media-admin.test.mjs` executed via `npm --prefix apps/next-admin run test:media`.
- **Verified Coverage**:
  - `mediaNavItems` configuration and routing parameters.
  - `formatBytes` boundary tests (`<0`, `0`, `KB`, `MB`, `GB`).
  - Strict tenant ID enforcement on data queries and deletion mutations.
  - Pagination boundary clamping and argument mapping.
  - GraphQL document compliance with backend schema.
