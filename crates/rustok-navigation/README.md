# rustok-navigation

## Purpose

Own localized navigation menus and deterministic tenant/channel/slot bindings.

## Responsibilities

- Menu and nested item persistence.
- Exact-locale public reads.
- Current-channel bindings for header, footer, sidebar and mobile locations.
- Navigation-owned GraphQL, HTTP and storefront UI surfaces.

## Entry points

- `NavigationModule`
- `NavigationQuery` / `NavigationMutation`
- `http::axum_router`
- `MenuService` / `MenuBindingService`

## Interactions

Navigation depends on Channel for current-channel scope. It does not depend on Pages; menu items store public URLs rather than owner-specific page identifiers.

See [module documentation](docs/README.md).
