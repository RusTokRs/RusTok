# RusToK Marketplace Seller Admin

Leptos FFA package for marketplace seller administration.

## Responsibilities

- Expose seller-owned admin routes and views.
- Consume the host-provided locale and route context.
- Keep native SSR and GraphQL-capable surfaces aligned.

## Interactions

The package calls Marketplace Seller owner contracts and shared UI transport libraries. Admin hosts provide composition and authentication context.

## Entry points

- `src/lib.rs` — public admin package surface.
- `src/transport.rs` — owner transport adapters.

See the [Marketplace Seller documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
