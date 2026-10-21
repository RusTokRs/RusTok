# rustok-commerce

## Purpose

`rustok-commerce` is the `Ecommerce` umbrella/root module for RusToK's commerce family.

## Responsibilities

- Provide `CommerceModule` metadata for the runtime registry.
- Serve as the umbrella entry point for the ecommerce family.
- Preserve the GraphQL surface while the Medusa-style REST transport expands.
- Expose the Medusa-style REST transport slice under `/store/*` and `/admin/*`.
- Resolve storefront cart line items from server-owned catalog/pricing data using `variant_id + quantity`, instead of trusting client-provided title and price.
- Orchestrate submodules of the ecommerce family through owner-crate services and typed ports, without masking owner APIs behind umbrella aliases.
- Own the checkout orchestration flow across cart, payment, order, and fulfillment submodules.
- Own store-context resolution across region, currency, and tenant locale policy.
- Apply channel-aware storefront availability on top of platform `ChannelContext` and `rustok-channel` bindings, without introducing a second sales-channel domain inside commerce.
- Apply shipping-profile compatibility between catalog products, storefront shipping discovery, cart context, and checkout validation, with typed product/variant bindings, typed line-item snapshots, and metadata normalization kept only as a backward-compatibility layer. An explicitly empty shipping-option `allowed_slugs` compatibility list represents an unrestricted option; malformed present compatibility metadata remains fail-closed.
- Expose first-class `shipping_profile_slug` on product and variant create/update/read contracts and `allowed_shipping_profile_slugs` on shipping-option contracts.
- Expose deliverability-aware cart and checkout contracts with `delivery_groups[]`, typed `shipping_selections[]`, `fulfillments[]`, and typed `fulfillment.items[]`, while keeping the old singular shipping/fulfillment fields only as single-group compatibility shims.
- Treat nullable `seller_id` as the canonical marketplace identity key across product, cart, order, checkout, and fulfillment contracts; `seller_scope` is not used as a runtime grouping or selection fallback.
- Expose admin/manual post-order fulfillment creation over REST and GraphQL with typed `items[]`, seller-aware delivery-group consistency checks, and remaining-quantity validation against order line items.
- Expose partial item-level `ship` / `deliver` adjustments over admin REST and GraphQL, with per-item shipped/delivered counters and a language-agnostic metadata-based audit trail.
- Expose explicit admin `reopen` / `reship` fulfillment recovery operations over REST and GraphQL, so post-order delivery corrections do not rely on implicit status rewrites.
- Expose admin return decision-tree transport over REST (`POST /admin/orders/{id}/returns/decision`) and GraphQL (`createOrderReturnDecision`) on top of `PostOrderOrchestrationService`, so `return_only` / `refund` / `exchange` orchestration stays service-owned.
- Keep the module-owned admin UI as an aggregate operator workspace for shipping profiles, cart promotions, and post-order order-change actions; exchange/claim apply/cancel actions call `orderChanges` / `applyOrderChange` / `cancelOrderChange` instead of embedding domain rules.
- Own the typed `shipping_profiles` registry and validate product/shipping-option references against active shipping profiles before write-path mutations are accepted.
- Resolve the effective shipping profile as `variant -> product -> default`, persist it into cart/order line-item snapshots, and use those snapshots instead of live product metadata for checkout deliverability decisions.
- Expose admin shipping-option management over REST and GraphQL (`list/show/create/update/deactivate/reactivate`) on top of `FulfillmentService`, so delivery compatibility and lifecycle are configurable without dropping to direct service calls.
- Expose admin shipping-profile management over REST and GraphQL (`list/show/create/update/deactivate/reactivate`) on top of `ShippingProfileService`.
- Expose only commerce-owned checkout/context/shipping-profile DTOs and shipping-profile entities; owner DTO/entity/service contracts are imported directly from `rustok-cart`, `rustok-customer`, `rustok-product`, `rustok-region`, `rustok-pricing`, `rustok-inventory`, `rustok-order`, `rustok-payment`, and `rustok-fulfillment`.
- Expose commerce-owned orchestration services such as `CheckoutService`, `StoreContextService`, `ShippingProfileService`, `PaymentOrchestrationService`, and `PostOrderOrchestrationService`.
- Keep shared/admin product, storefront product/catalog/order/cart/checkout, admin order/change/return, admin fulfillment, admin shipping, and admin payment HTTP handlers on narrow `CommerceHttpRuntime` state; remaining admin/storefront REST adapters are still cut over in separate owner-boundary slices.
- Keep commerce-owned orchestration code and leftover migrations not yet moved to new modules.
- Publish a module-owned Leptos admin UI package in `admin/` for host composition.
- Let the module-owned Leptos admin UI package keep the typed shipping-profile registry after product CRUD moved into `rustok-product/admin`, shipping-option UI moved into `rustok-fulfillment/admin`, order operations UI moved into `rustok-order/admin`, inventory visibility moved into `rustok-inventory/admin`, and pricing visibility moved into `rustok-pricing/admin`.
- Publish a module-owned Leptos storefront UI package in `storefront/` for host composition.
- Keep the aggregate storefront package focused on checkout orchestration while split modules publish their own storefront routes, with `rustok-region/storefront`, `rustok-product/storefront`, `rustok-pricing/storefront`, and `rustok-cart/storefront` already owning their public slices.
- Keep `rustok-commerce/storefront` focused on aggregate checkout workspace concerns such as delivery-group shipping selection, payment-collection reuse, and checkout completion over a selected cart.
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
- `apps/admin` consumes `rustok-commerce-admin` through manifest-driven `build.rs` code generation, with a module-owned commerce control room mounted under `/modules/commerce` for shipping-profile operations.
- `apps/admin` also consumes `rustok-fulfillment-admin` through the same manifest-driven composition path, with shipping-option CRUD and lifecycle now owned by the fulfillment module.
- `apps/admin` also consumes `rustok-order-admin` through the same manifest-driven composition path, with order list/detail/lifecycle now owned by the order module.
- `apps/admin` also consumes `rustok-inventory-admin` through the same manifest-driven composition path, with inventory visibility and stock-health inspection now owned by the inventory module.
- `apps/admin` also consumes `rustok-pricing-admin` through the same manifest-driven composition path, with pricing visibility and sale-marker inspection now owned by the pricing module.
- `apps/admin` also consumes `rustok-product-admin` through the same manifest-driven composition path, with catalog CRUD now owned by the product module instead of the aggregate commerce route.
- `apps/storefront` consumes `rustok-commerce-storefront` through manifest-driven `build.rs` code generation, with the aggregate checkout workspace mounted under `/modules/commerce`.
- `apps/storefront` also consumes `rustok-product-storefront` through the same manifest-driven composition path, with published catalog discovery now owned by the product module under `/modules/products`.
- `apps/storefront` also consumes `rustok-pricing-storefront` through the same manifest-driven composition path, with public pricing discovery now owned by the pricing module under `/modules/pricing`.
- `apps/storefront` also consumes `rustok-region-storefront` through the same manifest-driven composition path, with public region discovery mounted under `/modules/regions`.
- `apps/storefront` also consumes `rustok-cart-storefront` through the same manifest-driven composition path, with cart inspection and safe line-item decrement/remove actions mounted under `/modules/cart`.
- `rustok-module.toml` exports both surfaces through `[provides.admin_ui]` and `[provides.storefront_ui]`, so host wiring stays manifest-derived instead of relying on manual route registration.
- Declares permissions via `rustok-core::Permission` for `products`, `orders`, `customers`,
  `regions`, `payments`, `fulfillments`, `inventory`, and `discounts`.
- Transport adapters validate permissions against `AuthContext.permissions`, then invoke
  commerce services or direct tenant-scoped SeaORM reads where the module still owns the
  read-model assembly; post-order order-change preview/apply/cancel transport and
  return decision-tree transport stay backed by module services (`rustok-order::OrderService`
  and `PostOrderOrchestrationService`) rather than host-owned logic.
- Authenticated principals that lack a required Commerce Admin permission receive HTTP `403 Forbidden`; HTTP `401 Unauthorized` is reserved for missing authentication context, and protected Admin OpenAPI operations advertise both responses.
- Store guest-cart reads and mutations require the cart's server-issued guest capability; anonymous HTTP actor classification never substitutes for that capability, and cart-scoped Store operations advertise `403 Forbidden` when the capability is missing or invalid.
- Authenticated access to another customer's cart or order is an authorization denial (`403 Forbidden`); unauthenticated access to a customer-owned resource remains `401 Unauthorized`.
- Channel-aware price resolution is intentionally not part of the current storefront availability baseline and remains planned under Pricing 2.0.

## Capability classification gap

The current module graph declares Fulfillment as an unconditional Commerce dependency,
and current checkout assumes delivery groups for the shipping-aware path. That is
current executable truth, not the accepted target for all product kinds. Digital
products and digital order lines require no shipping capability; Commerce must remain
available for a digital-only flow when Fulfillment is absent.

The cutover must preserve a typed fulfillment requirement in product/cart/order
snapshots, form delivery groups only for physical lines, and require Fulfillment at
the affected operation boundary. Mixed carts must not assign a synthetic shipping
profile to digital lines. See
[`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

## Post-order lifecycle orchestration (2026-10-21, unverified)

- Admin order `cancel` is routed through `OrderCancellationOrchestrator` (decorator of
  `OrderAdminCommandPort`): it cancels live fulfillments, voids authorized collections, requests a
  full refund of captured money and only then cancels the order. Orders with shipped items are
  rejected (use a return). See `DECISIONS/2026-10-21-order-cancellation-orchestration.md`.
- Return refunds are valued on the server (`return_refund_value.rs`) from the order line price,
  discounts and tax; an explicit amount may only be lower. Physical lines cannot be returned beyond
  their shipped quantity (cumulative over live returns) when fulfillment records exist. See
  `DECISIONS/2026-10-21-return-refund-server-side-valuation.md`.
- Shipment moves stock in the fulfillment transaction (ship consumes, cancel of a shipped fulfillment
  restores, reopen of it reconsumes) and the operator can restock returned goods at return
  completion. See `DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md`.
- Order `shipped`/`delivered` follow fulfillment truth (`OrderShipmentProjector`, decorator of
  `FulfillmentAdminCommandPort`); manual order `ship`/`deliver` are refused while live fulfillments
  contradict them. See `DECISIONS/2026-10-21-order-shipment-status-projection.md`.

### Known Limitations / Pending Implementation

- This code was written without a Rust toolchain: it has not been compiled, formatted or tested.
  Run `cargo test -p rustok-commerce` (crate-scoped) before relying on it.
- Cancellation has no durable journal; it is a stateless re-runnable saga. The cancellation refund
  is created `pending` and is completed by the provider or operator.
- `complete_return` restocks only on operator decision (`metadata.restock = "all"` or
  `[{return_item_id, quantity}]`) and only through `ReturnCompletionOrchestrationService`; return
  completion paths that bypass it never touch stock. Restock goes to the item and location of the
  order line's reservation; lines without a reservation are skipped.
- Return records can still be created on unpaid orders (a refund needs a captured collection).
- Shipment projection skips fulfillment changes made outside the admin command port (provider
  reconciliation) and never moves an order status backwards on reopen/reship.
- Stock moves on shipment through the fulfillment-owned `ShipmentStockPort`, composed here as
  `InventoryShipmentStockPort`; every ship-capable fulfillment service in this crate must be built
  with `fulfillment_service(db)` / `fulfillment_admin_command_runtime(db, registry)`. A bare
  `FulfillmentService::new` fails ship / cancel-shipped / reopen-cancelled on purpose. Migration
  `m20261021_000001` drops the former trigger and must run before the new binary takes traffic. See
  `DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md`. Provider reconciliation that
  applies results outside the fulfillment service does not move stock.

## Entry points

- `CommerceModule`
- `ShippingProfileService`
- `CheckoutService`
- `StoreContextService`
- `PaymentOrchestrationService`
- `PostOrderOrchestrationService`
- `graphql::CommerceQuery`
- `graphql::CommerceMutation`
- `controllers::routes`
- `admin::CommerceAdmin` (publishable Leptos package)
- `storefront::CommerceView` (publishable Leptos package)
- commerce-owned `dto` and `entities` modules; owner DTO/entity/service contracts are imported from owner crates directly

See also `docs/README.md`.


## Storefront Product HTTP policy (2026-10-19)

Mounted Product list/detail share a static presentation policy. **Forbidden now returns
403 and Conflict 409 instead of the prior generic 500**; missing/invisible Product
remains 404. Mapper diagnostics are bounded and owner-call correlations are opaque.
See the [contract and migration note](docs/public-port-error-safety.md#storefront-product-http-policy-continuation-2026-10-19).
Source guards/format checks pass; four Rust scenarios are unrun and compilation,
mounted execution, tracing capture and integration remain open.
