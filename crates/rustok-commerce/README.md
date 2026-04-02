# rustok-commerce

## Purpose

`rustok-commerce` is the `Ecommerce` umbrella/root module for RusToK's commerce family.

## Responsibilities

- Provide `CommerceModule` metadata for the runtime registry.
- Serve as the umbrella entry point for the ecommerce family.
- Preserve the GraphQL surface while the Medusa-style REST transport expands.
- Expose the Medusa-style REST transport slice under `/store/*` and `/admin/*`.
- Resolve storefront cart line items from server-owned catalog/pricing data using `variant_id + quantity`, instead of trusting client-provided title and price.
- Orchestrate submodules of the ecommerce family through the compatibility layer.
- Own the checkout orchestration flow across cart, payment, order, and fulfillment submodules.
- Own store-context resolution across region, currency, and tenant locale policy.
- Apply channel-aware storefront availability on top of platform `ChannelContext` and `rustok-channel` bindings, without introducing a second sales-channel domain inside commerce.
- Apply metadata-backed shipping-profile compatibility between catalog products, storefront shipping discovery, cart context, and checkout validation.
- Expose first-class `shipping_profile_slug` on product create/update/read contracts while the underlying storage still uses the metadata-backed shipping profile shape.
- Re-export the shared DTO/entity/error surface from `rustok-commerce-foundation`.
- Re-export `CartService`, `CustomerService`, `CatalogService`, `PricingService`, `InventoryService`, `OrderService`, `PaymentService`, `FulfillmentService`, and `CheckoutService` from the split modules and orchestration layer.
- Re-export `RegionService` and `StoreContextService` from the region submodule and umbrella policy layer.
- Keep commerce-owned orchestration code and leftover migrations not yet moved to new modules.
- Publish a module-owned Leptos admin UI package in `admin/` for host composition.
- Publish a module-owned Leptos storefront UI package in `storefront/` for host composition.
- Publish the typed RBAC surface for commerce resources.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `rustok-commerce-foundation` for shared DTOs, entities, search helpers, and errors.
- Depends on `rustok-cart`, `rustok-customer`, `rustok-product`, `rustok-region`, `rustok-pricing`,
  `rustok-inventory`, `rustok-order`, `rustok-payment`, and `rustok-fulfillment` as the default cart,
  customer, product, region, pricing, inventory, order, payment, and fulfillment submodules of the ecommerce family.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Depends on `rustok-channel` for platform-level channel bindings and request-aware storefront visibility rules.
- Depends on `rustok-outbox` and `rustok-events` for transactional domain-event publishing.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- `apps/admin` consumes `rustok-commerce-admin` through manifest-driven `build.rs` code generation, with a module-owned catalog control room mounted under `/modules/commerce`.
- `apps/storefront` consumes `rustok-commerce-storefront` through manifest-driven `build.rs` code generation, with a public catalog surface mounted under `/modules/commerce`.
- `rustok-module.toml` exports both surfaces through `[provides.admin_ui]` and `[provides.storefront_ui]`, so host wiring stays manifest-derived instead of relying on manual route registration.
- Declares permissions via `rustok-core::Permission` for `products`, `orders`, `customers`,
  `regions`, `payments`, `fulfillments`, `inventory`, and `discounts`.
- Transport adapters validate permissions against `AuthContext.permissions`, then invoke
  commerce services or direct tenant-scoped SeaORM reads where the module still owns the
  read-model assembly.

## Entry points

- `CommerceModule`
- `CartService`
- `CustomerService`
- `CatalogService`
- `PricingService`
- `InventoryService`
- `RegionService`
- `OrderService`
- `PaymentService`
- `FulfillmentService`
- `CheckoutService`
- `StoreContextService`
- `graphql::CommerceQuery`
- `graphql::CommerceMutation`
- `controllers::routes`
- `admin::CommerceAdmin` (publishable Leptos package)
- `storefront::CommerceView` (publishable Leptos package)
- commerce DTO and state-machine re-exports

See also `docs/README.md`.
