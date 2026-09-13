# RusToK Brand Admin

Leptos FFA package for brand catalog and product brand relationship administration.

## Responsibilities

- Expose brand-owned admin routes and views.
- Consume the host-provided locale and route context.
- Keep native SSR and GraphQL-capable surfaces aligned.
- Manage brand lifecycle, metadata, localized presentation, and product associations.

## Interactions

The package calls Brand owner contracts and shared UI transport libraries. Admin hosts provide composition and authentication context.

## Entry points

- `src/lib.rs` — public admin package surface.
- `src/transport.rs` — owner transport adapters.
- `src/ui/leptos.rs` — Leptos UI components and `BrandAdmin` root view.

See the [Brand documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
