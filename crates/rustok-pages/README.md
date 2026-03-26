# rustok-pages

## Purpose

`rustok-pages` owns static pages, blocks, and menus for RusToK.

## Responsibilities

- Provide `PagesModule` metadata for the runtime registry.
- Own page, block, and menu services layered on top of content storage.
- Own the Pages GraphQL and REST adapters exported from the module crate.
- Publish the module-owned Leptos admin and storefront root packages.
- Keep one real module-owned Leptos vertical slice for pages list/create/edit/update/publish/delete
  in admin and slug-driven published-page rendering in storefront.
- Publish the typed RBAC surface for `pages:*` and the node-based block helpers it needs.

## Interactions

- Depends on `rustok-content` for shared node storage and content helpers.
- Depends on `rustok-channel` for the first public channel-aware gating proof point on pages read paths and the first page-level publication proof point via `channelSlugs`.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared tenant/auth/request/GraphQL helper contracts.
- Used by `apps/server` as a composition-root dependency; server now re-exports module-owned pages GraphQL and REST entry points.
- Used by `apps/admin` through `rustok-pages-admin` and by `apps/storefront` through `rustok-pages-storefront`.
- Pages GraphQL now defaults tenant resolution from `TenantContext`, so module-owned UI packages do
  not need to carry tenant UUIDs through the host boundary.
- `rustok-pages-storefront` also consumes the shared `UiRouteContext`, so package-owned storefront
  screens can resolve locale/query-based state without teaching the host about pages specifics.
- Public pages read paths can now honor `channel_module_bindings` when a request carries an active
  channel through `RequestContext`; authenticated/admin flows intentionally bypass that pilot gate.
- Public pages read paths also honor page-level `channelSlugs` allowlists stored in metadata for
  unauthenticated published requests; empty allowlists stay globally visible, while authenticated/admin
  flows intentionally bypass this experimental publication gate.
- Declares permissions via `rustok-core::Permission`.
- Module adapters enforce `pages:*` permissions from `AuthContext.permissions` and pass a
  permission-aware `SecurityContext` into page services.

## Entry points

- `PagesModule`
- `PageService`
- `BlockService`
- `MenuService`
- `graphql::PagesQuery`
- `graphql::PagesMutation`
- `controllers::routes`
- `rustok-pages-admin::PagesAdmin`
- `rustok-pages-storefront::PagesView`
