# rustok-commerce

## Purpose

`rustok-commerce` is the `Ecommerce` umbrella/root module for RusToK's commerce family.

## Responsibilities

- Provide `CommerceModule` metadata for the runtime registry.
- Serve as the umbrella entry point for the ecommerce family.
- Preserve the legacy GraphQL and REST transport surface during the transition.
- Orchestrate submodules of the ecommerce family through the compatibility layer.
- Re-export the shared DTO/entity/error surface from `rustok-commerce-foundation`.
- Re-export `CatalogService`, `PricingService`, and `InventoryService` from the split modules.
- Keep legacy commerce-owned state-machine and leftover migrations not yet moved to new modules.
- Publish the typed RBAC surface for commerce resources.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `rustok-commerce-foundation` for shared DTOs, entities, search helpers, and errors.
- Depends on `rustok-product`, `rustok-pricing`, and `rustok-inventory` as the default
  product, pricing, and inventory submodules of the ecommerce family.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Depends on `rustok-outbox` and `rustok-events` for transactional domain-event publishing.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- Declares permissions via `rustok-core::Permission` for `products`, `orders`, `customers`,
  `inventory`, and `discounts`.
- Transport adapters validate permissions against `AuthContext.permissions`, then invoke
  commerce services or direct tenant-scoped SeaORM reads where the module still owns the
  read-model assembly.

## Entry points

- `CommerceModule`
- `CatalogService`
- `PricingService`
- `InventoryService`
- `graphql::CommerceQuery`
- `graphql::CommerceMutation`
- `controllers::routes`
- commerce DTO and state-machine re-exports

See also `docs/README.md`.
