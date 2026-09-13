# RusToK Product Bundles Admin

Leptos FFA package for product bundles, kits, and configurable sets administration.

## Responsibilities

- Expose product bundles admin routes and views.
- Consume the host-provided locale and route context.
- Keep native SSR and GraphQL-capable surfaces aligned.
- Manage bundle lifecycle, status, bundle types, package discounts, and component item compositions.

## Interactions

The package calls Product Bundles owner contracts and shared UI transport libraries. Admin hosts provide composition and authentication context.

## Entry points

- `src/lib.rs` — public admin package surface.
- `src/transport.rs` — owner transport adapters.
- `src/ui/leptos.rs` — Leptos UI components and `BundleAdmin` root view.

See the [Product Bundles documentation](../README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
