# RusToK Product Relations Admin

Leptos FFA package for product relations and merchandising associations administration.

## Responsibilities

- Expose product relations administration routes, views, and embeddable panels (`ProductRelationsPanel`, `ProductRelationsAdmin`).
- Manage cross-sells, up-sells, related products, accessories, and alternatives.
- Support reordering and position adjustment for relation groups.
- Consume the host-provided locale and route context.
- Keep native SSR and GraphQL-capable surfaces aligned.

## Interactions

The package calls Product Relations owner contracts and shared UI transport libraries. Admin hosts provide composition and authentication context.

## Entry points

- `src/lib.rs` — public admin package surface (exports `ProductRelationsPanel` and `ProductRelationsAdmin`).
- `src/transport.rs` — owner transport adapters.
- `src/ui/leptos.rs` — Leptos UI components.

See the [Product Relations documentation](../README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
