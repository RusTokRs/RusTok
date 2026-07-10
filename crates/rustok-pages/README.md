# rustok-pages

## Purpose

`rustok-pages` owns static pages, page bodies, legacy blocks, and menus for RusToK.

## Responsibilities

- Provide `PagesModule` metadata for the runtime registry.
- Own page storage across `pages`, `page_translations`, `page_bodies`, `page_blocks`, and `page_channel_visibility`.
- Own menu storage across `menus`, `menu_translations`, `menu_items`, and `menu_item_translations`.
- Own the Pages GraphQL and REST adapters exported from the module crate.
- Keep REST page/block handlers on narrow `PagesHttpRuntime` state; the manifest-declared Axum router builds it from `HostRuntimeContext` and a typed transactional event bus.
- Publish the module-owned Leptos admin and storefront root packages.
- Keep one real module-owned Leptos vertical slice for pages list/create/edit/update/publish/delete
  in admin and slug-driven published-page rendering in storefront.
- Publish the typed RBAC surface for `pages:*`.

## Interactions

- Depends on `rustok-content` for shared content helpers and on `rustok-page-builder` for builder capability contracts (`preview/tree/properties/publish`).
- Builder rollout is controlled by manifest metadata: tenant switches must write before/after snapshots, keep/rollback decisions, and owner sign-off to `control_plane_builder_wave_audit`; pilot tenants must smoke `preview -> properties -> publish(dry)`; rollback target is <= 10 minutes without redeploy.
- Depends on `rustok-channel` for the first public channel-aware gating proof point on pages read paths and typed page-level channel visibility via `channelSlugs`.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared tenant/auth/request/GraphQL helper contracts.
- Used by `apps/server` as a composition-root dependency; generated composition merges its module-owned Axum router and GraphQL entry points.
- Used by `apps/admin` through `rustok-pages-admin` and by `apps/storefront` through `rustok-pages-storefront`.
- Pages GraphQL now defaults tenant resolution from `TenantContext`, so module-owned UI packages do
  not need to carry tenant UUIDs through the host boundary.
- `rustok-pages-storefront` also consumes the shared `UiRouteContext`, so package-owned storefront
  screens can resolve locale/query-based state without teaching the host about pages specifics.
- Public pages read paths can now honor `channel_module_bindings` when a request carries an active
  channel through `RequestContext`; authenticated/admin flows intentionally bypass that pilot gate.
- Public pages read paths also honor page-level `channelSlugs` visibility stored in
  module-owned `page_channel_visibility`; empty allowlists stay globally visible, while
  authenticated/admin flows intentionally bypass this publication gate.
- `rustok-pages` deliberately has no default integration with `rustok-comments`; commentable
  page-like surfaces, if needed later, must be explicit opt-in product slices.
- Page builder compatibility is explicit: `body.format = "grapesjs_v1"` is the canonical
  visual-builder payload, while legacy `blocks` remain an independent migration surface.
- Pages may legitimately exist with legacy blocks and no `body`; adding or updating a body does
  not auto-convert, overwrite, or delete existing blocks. This read/bridge invariant is checked
  without compilation by `verify-page-builder-pages-legacy-bridge.mjs`.
- Page CRUD, localized page metadata, visual-builder bodies, legacy blocks, and page-channel visibility run on module-owned tables: `pages`, `page_translations`, `page_bodies`, `page_blocks`, and `page_channel_visibility`.
- Menu CRUD and localized menu trees run on module-owned tables: `menus`, `menu_translations`, `menu_items`, and `menu_item_translations`.
- Declares permissions via `rustok-core::Permission`.
- Module adapters enforce `pages:*` permissions from `AuthContext.permissions` and pass a
  permission-aware `SecurityContext` into page services.
- Page, block, and menu services now re-validate `pages:*` locally; `publish` can no longer be
  bypassed through create/update flows, customer read paths only see published pages, and admin/authenticated service paths intentionally bypass public channel visibility allowlists.

## Entry points

- `PagesModule`
- `PageService`
- `BlockService`
- `MenuService`
- `graphql::PagesQuery`
- `graphql::PagesMutation`
- `controllers::axum_router`
- `rustok-pages-admin::PagesAdmin`
- `rustok-pages-storefront::PagesView`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
