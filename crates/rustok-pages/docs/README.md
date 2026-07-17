# `rustok-pages` Documentation

`rustok-pages` — domain module for pages, menus and visual page-builder flows.
The module already works on pages-owned storage and must remain the owner of page,
block and menu contracts.

## Purpose

- publish the canonical pages runtime contract for page/body/block/menu surfaces;
- keep module-owned transport adapters and UI packages inside the module;
- evolve pages as a channel-aware module without reverting to shared node storage.

## Scope

- `PageService`, `BlockService`, `MenuService` and page visibility semantics;
- module-owned storage for `pages`, `page_translations`, `page_bodies`, `page_blocks`, `page_channel_visibility`, `menus`, `menu_translations`, `menu_items`, `menu_item_translations`;
- GraphQL/REST adapters and Leptos admin/storefront packages;
- REST page/block handlers consume narrow `PagesHttpRuntime` state with explicit DB/event bus handles; `controllers::axum_router` builds it from `HostRuntimeContext` and generated host composition mounts it without a framework adapter;
- canonical write-path for visual builder via `body.format = "grapesjs"`;
- typed relation `page_channel_visibility` for publication-level visibility.

## Integration

- uses `rustok-content` only for shared rich-text helpers, not as a storage backend;
- depends on the capability module `rustok-page-builder` for FBA builder-contract (`preview/tree/properties/publish`) and corresponding degraded/toggle profiles;
- uses `rustok-channel` for module-level and publication-level visibility contract;
- host applications connect pages UI through manifest-driven generated wiring;
- `rustok-pages/admin` already embeds owner-side page SEO panel via `rustok-seo-admin-support`
  and the shared capability contract of the `rustok-seo` module;
- block endpoints remain a migration-compatible surface and must not implicitly synthesize `body`; legacy `blocks` are considered read/bridge compatibility for visual-builder rollout: import/create is preserved, but `grapesjs` body writes do not delete blocks and do not extend the block write surface;
- FBA rollout policy for the builder capability layer is stored in `rustok-module.toml`: tenant flags `builder.enabled`, `builder.preview.enabled`, `builder.properties.enabled`, `builder.publish.enabled` switch without redeploying pages runtime; `control_plane_builder_wave_audit` must store before/after snapshots, keep/rollback decision, owner sign-off, SLO rollback triggers and pilot smoke `preview -> properties -> publish(dry)`.
- The pages runtime contour remains owner of the page/menu/visibility/publish contract, while the external `rustok-page-builder` remains the provider of visual capability surfaces; reverting to pages-local ownership of the editor runtime is prohibited by module metadata and FBA baseline gate.

## Verification

- `cargo xtask module validate pages`
- `cargo xtask module test pages`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:pages:legacy-bridge`
- targeted tests for page/block/menu flows, grapesjs body contract, degraded builder profiles, RBAC/admin bypass and channel visibility semantics
- `npm run verify:page-builder:error-catalog`

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
