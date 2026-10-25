# rustok-navigation-storefront

Module-owned header and footer navigation contributions for the RusToK Storefront.

## Responsibilities

- Render responsive header and footer navigation menus via Fluid Frontend Architecture (FFA) slots.
- Dual-path transport support: native Leptos Server Functions (`ssr`) and headless GraphQL queries.
- Storefront navigation snapshot state management (`StorefrontNavigationSnapshot`).
- Active menu fetching based on channel slot and locale (`fetch_active_menu`).

## Entry Points

- `NavigationHeaderMenu`: Top-level storefront navigation menu component.
- `NavigationSnapshotProvider`: Context provider for storefront navigation tree snapshot.
- `NavigationView`: View renderer for a specific navigation menu location.
- `fetch_active_menu`: Transport loader for active menu location.

For the formal Rust API contract and guarantees, see [`CRATE_API.md`](CRATE_API.md).
