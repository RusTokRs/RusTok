# RusToK Marketplace Listing Admin

Leptos FFA package for marketplace listing administration.

## Responsibilities

- Expose listing-owned admin routes and views.
- Use host locale and transport context.
- Keep SSR and hydration behavior aligned.

## Interactions

The package consumes Marketplace Listing owner contracts and shared UI/GraphQL libraries. Hosts compose its exported entry points.

## Entry points

- `src/lib.rs` — public admin package surface.
- `src/transport.rs` — native and GraphQL transport adapters.

See the [Marketplace Listing documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
