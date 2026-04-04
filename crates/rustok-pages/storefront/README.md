# rustok-pages-storefront

## Purpose

`rustok-pages-storefront` publishes the Leptos storefront root view for the `rustok-pages` module.

## Responsibilities

- Export the module-owned `PagesView` root component for `apps/storefront`.
- Keep pages-specific storefront rendering inside the module boundary.
- Act as the canonical working storefront read-path for published pages.

## Interactions

- Used by `apps/storefront` through manifest-driven generated wiring.
- Uses dual-path data access: native Leptos `#[server]` functions first, then GraphQL fallback.
- Native `#[server]` path goes from the storefront host into `PageService` and DB without removing GraphQL.
- Keeps the pages module GraphQL read contract active in parallel with the native path while rendering a slug-selected page and a small page directory.
- Follows the generic storefront host contract: slots plus locale-aware module routes built from `UiRouteContext::module_route_base()`.

## Entry points

- `PagesView`
