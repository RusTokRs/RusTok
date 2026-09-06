# Documentation `rustok-commerce`

This folder contains the documentation for the umbrella module `crates/rustok-commerce`.

## Purpose

- maintain `rustok-commerce` as the umbrella/root module for the ecommerce family;
- hold orchestration, transport and cross-domain contracts that have not yet been extracted into split modules;
- prevent domain ownership from being returned from split modules back to the host layer.

## Scope

- orchestration between `cart/customer/product/region/pricing/inventory/order/payment/fulfillment`;
- REST/GraphQL transport and aggregate orchestration UI surfaces that remain commerce-owned after domain surfaces move to ownership boundaries;
- shared/admin product, storefront product/catalog/order/cart/checkout, admin order/change/return, admin fulfillment, admin shipping and admin payment HTTP handlers on a narrow `CommerceHttpRuntime`; remaining admin/storefront REST adapters are cut by separate owner-boundary slices;
- channel-aware commerce contract over `rustok-channel`, checkout orchestration and cross-domain deliverability semantics;
- maintaining the thin-host role of `apps/server` without returning commerce business logic to the host.

## Integration

- `apps/server` remains the adapter/wiring layer for route, OpenAPI and schema composition;
- split ecommerce modules own their persistence/runtime boundaries, while `rustok-commerce` coordinates cross-domain flow;
- module-owned UI packages are connected by host applications through manifest-driven composition;
- any cross-domain contract changes must be synchronized with the local docs of split modules and the platform central docs.

## Verification

Baseline verification gates for the current module state:

- `cargo xtask module validate commerce`
- `cargo xtask module test commerce`
- `cargo test -p rustok-commerce admin_order_transport_returns_order_with_payment_and_fulfillment -- --exact`
- `cargo test -p rustok-commerce storefront_graphql_customer_and_order_queries_match_customer_owned_read_path -- --exact`
- `cargo test -p rustok-order order_tax_lines_insert_without_provider_id_use_region_default -- --exact`

Note: when changing runtime wiring or transport contracts, targeted parity tests must be run
for checkout, REST/GraphQL transport and split-module integration in addition to the baseline gate commands.

## Related documents

- [Implementation plan](./implementation-plan.md) — current roadmap for ecommerce family development, Medusa-style REST transport, channel-aware commerce over `rustok-channel` and responsibility extraction into separate modules.
- [RusTok vs Medusa comparison](../../../docs/research/medusa-vs-rustok-architecture.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)

## Current state

- `rustok-commerce` remains the umbrella/root module for the ecommerce family and holds orchestration, transport and the remaining uncut parts of the domain.
- The main REST contract lives on `/store/*` and `/admin/*`; legacy `/api/commerce/*` has been removed from the live route tree and OpenAPI.
- On the admin surface, besides product management, there are already paginated order transport (`GET /admin/orders`, `GET /admin/orders/{id}`), explicit order lifecycle routes (`mark-paid`, `ship`, `deliver`, `cancel`), list/detail/lifecycle routes for `payment-collections`, `refunds`, `fulfillments`, order-change preview/apply/cancel (`/admin/orders/{id}/changes`, `/admin/order-changes*`) and return decision tree (`POST /admin/orders/{id}/returns/decision`) with `return_only/refund/exchange/claim`, plus a manual post-order `create fulfillment` route with typed `items[]`.
- GraphQL surface is preserved and uses the same application services as REST; for admin commerce there is parity on order/payment/fulfillment/order-change queries, including list read-path for `paymentCollections`, `fulfillments` and `orderChanges`, lifecycle mutations, `createOrderChange`, `createOrderReturnDecision` (`return_only/refund/exchange/claim`) and manual `createFulfillment`, while the storefront surface now includes `storefrontRegions`, `storefrontShippingOptions`, `storefrontCart`, `createStorefrontCart`, `updateStorefrontCartContext`, cart line-item lifecycle, `storefrontMe`, customer-owned `storefrontOrder`, `createStorefrontPaymentCollection`, `completeStorefrontCheckout`, as well as pricing-facing read helpers `storefrontPricingChannels`, `storefrontActivePriceLists(channelId, channelSlug)`, `storefrontPricingProduct` and `adminPricingProduct` for module-owned fallback surfaces.
- Generic catalog roots `product` / `storefrontProduct` should now be treated only as a catalog-authoritative surface: their `variants.prices` remains a compatibility snapshot without explicit currency/region/price-list/channel resolution and is not considered a pricing source of truth alongside dedicated pricing roots.
- `apps/server` remains a thin host layer: routes, OpenAPI and schema composition, without duplication of commerce business logic.
- Cart snapshot already stores storefront context (`region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`, `customer_id`, `email`, `currency_code`) and channel snapshot (`channel_id`, `channel_slug`); the same channel snapshot is now carried over into order transport on checkout.
- The checkout flow uses `checking_out`, reuse payment collection and recovery semantics for repeated storefront requests.
- The platform already passes `ChannelContext` through `rustok-api` and `apps/server`, and `commerce` has started using this layer as a real storefront input: `/store/*` and storefront GraphQL now respect `channel_module_bindings`, and catalog/shipping visibility can be restricted with a metadata-based allowlist by `channel_slug`.
- Storefront product detail, cart mutation path and checkout validation now consider not only channel-aware product and shipping option visibility, but also available inventory by stock locations visible for the current `channel_slug`; stale cart no longer passes checkout with a hidden product or with stock already unavailable for the channel.
- For shipping profiles, the metadata-backed baseline is no longer the sole source of truth: `commerce` now has a typed registry `shipping_profiles` + `ShippingProfileService`, and `products.shipping_profile_slug` and `product_variants.shipping_profile_slug` now live as typed persistence with backward-compatible normalization in metadata.
- The product catalog surface additionally exposes first-class `shipping_profile_slug`, the shipping option surface exposes first-class `allowed_shipping_profile_slugs`, and the admin/storefront write-path now validates these references against the active typed shipping-profile registry.
- Cart and checkout are now also deliverability-aware: line items, `cart_shipping_selections`, order line items and fulfillment metadata store canonical language-agnostic seller identity (`seller_id`); cart delivery grouping and shipping selection do not use `seller_scope` as a fallback, cart response returns seller-aware `delivery_groups[]`, cart context/checkout accept typed `shipping_selections[]`, and checkout creates `fulfillments[]` with one entry per delivery group with typed `fulfillment.items[]`.
- The post-order admin create path now also relies on typed `fulfillment.items[]`: manual follow-up fulfillments validate `order_line_item_id` against the order, prevent exceeding remaining quantity, maintain seller-aware delivery-group boundary and propagate the same invariant to REST/GraphQL.
- Admin lifecycle transport is no longer coarse-only for fulfillments: `ship` and `deliver` can now accept item-level quantity adjustments, `fulfillment.items[]` return `shipped_quantity` / `delivered_quantity` together with language-agnostic audit trail in metadata, and explicit post-order recovery actions `reopen` / `reship` have been added on top; the free-form `delivered_note` remains a typed field rather than being duplicated in JSON audit.
- Legacy single-group contract is preserved only as a compatibility shortcut: `selected_shipping_option_id`, singular `shipping_option_id` and singular `fulfillment` are only filled for carts with one delivery group.
- Preflight validation in checkout now fires before side effects: stale shipping-profile snapshot, missing per-group selection or incompatible shipping option release the `checking_out` lock and do not create payment/order artifacts.
- Admin REST and admin GraphQL now also have a typed shipping-option management surface: `list/show/create/update/deactivate/reactivate` for shipping options over `FulfillmentService`, including `allowed_shipping_profile_slugs` and lifecycle via `active`.
- Admin REST and admin GraphQL now also have a typed shipping-profile management surface: `list/show/create/update/deactivate/reactivate` over `ShippingProfileService`, so compatibility rules no longer live only in metadata or service helpers.
- The module-owned admin UI package `rustok-commerce/admin` no longer holds product CRUD or shipping-option UI and has been narrowed to typed shipping-profile registry, aggregate cart promotions and post-order operator surfaces.
- The module-owned admin UI package `rustok-fulfillment/admin` has taken shipping-option lifecycle and compatibility UX under the ownership boundary of the `fulfillment` module.
- The module-owned admin UI package `rustok-customer/admin` has taken customer list/detail/create/update UX under the ownership boundary of the `customer` module and uses native Leptos server functions instead of a new umbrella transport.
- The module-owned admin UI package `rustok-region/admin` has taken region list/detail/create/update UX under the ownership boundary of the `region` module and uses native Leptos server functions over `RegionService`.
- The module-owned storefront UI package `rustok-region/storefront` has taken public region discovery UX under the ownership boundary of the `region` module, using native Leptos server functions with a GraphQL selected path over `storefrontRegions`.
- The module-owned storefront UI package `rustok-product/storefront` has taken published catalog discovery UX under the ownership boundary of the `product` module, using native Leptos server functions over `CatalogService` and preserving GraphQL as the storefront selected path.
- The module-owned storefront UI package `rustok-pricing/storefront` has taken public pricing atlas UX under the ownership boundary of the `pricing` module, using native Leptos server functions over `PricingService` and preserving GraphQL storefront contract as fallback.
- The module-owned storefront UI package `rustok-cart/storefront` has taken storefront cart inspection UX and safe decrement/remove line-item actions under the ownership boundary of the `cart` module, using native Leptos server functions over `CartService` and preserving GraphQL storefront contract as fallback.
- The aggregate storefront UI package `rustok-commerce/storefront` no longer duplicates catalog/pricing discovery and has been reduced to an aggregate checkout workspace: it shows effective storefront context, checkout state by `?cart_id=` and the remaining aggregate actions for seller-aware delivery-group shipping selection, `payment collection` and `complete checkout`, while discovery/edit surfaces already live in split storefront packages.
- A minimal multivendor foundation has been established in ecommerce: the product create/update contract now accepts nullable `seller_id`, grouping and ownership validation rely on `seller_id`, and seller display data is no longer persisted as a source of truth in ecommerce storage.
- The module-owned admin UI package `rustok-order/admin` has taken order list/detail/lifecycle UX under the ownership boundary of the `order` module.
- The module-owned admin UI package `rustok-inventory/admin` has taken inventory visibility and stock-health UX under the ownership boundary of the `inventory` module; the inventory-owned native read path is the only admin read transport, the former commerce GraphQL adapter has been removed, and set/adjust/reserve/release quantity and check-availability flows use an inventory-owned native write/validation surface without a GraphQL selected path.
- The module-owned admin UI package `rustok-pricing/admin` has taken pricing visibility and sale-marker UX under the ownership boundary of the `pricing` module, keeping the transport gap explicitly documented.
- Publishable UI packages for admin/storefront live inside the module and are connected by host applications through manifest-driven composition.

## Near-term roadmap

- The UI split is already underway and the storefront-side phase is also advanced: product admin route lives in `rustok-product/admin`, shipping options have moved to `rustok-fulfillment/admin`, customer operations have moved to `rustok-customer/admin`, order operations have moved to `rustok-order/admin`, inventory visibility and targeted stock/reservation/availability actions have moved to `rustok-inventory/admin`, pricing visibility has moved to `rustok-pricing/admin`, region CRUD has moved to `rustok-region/admin`, public region discovery has moved to `rustok-region/storefront`, published catalog discovery has moved to `rustok-product/storefront`, public pricing atlas has moved to `rustok-pricing/storefront`, storefront cart inspection and safe decrement/remove actions have moved to `rustok-cart/storefront`, `rustok-commerce-admin` has been left for shipping-profile registry, aggregate cart promotions and post-order operator surfaces, and `rustok-commerce-storefront` now holds the aggregate checkout workspace with seller-aware delivery-group shipping selection.
- The next step is no longer about grouping, basic fulfillment-item model, manual create path or partial ship/deliver baseline: stricter delivery audit trail, explicit `reopen` / `reship` semantics and the initial refund slice are already completed, so what remains is transport publication for the return decision tree and a broader post-order OMS surface (`exchanges/claims/order changes`).
- The cross-cutting track `Marketplace Foundations` is now active in parallel with phases `7-12`: the nearest scope is limited to stable `seller_id`, seller-owned product/catalog ownership contract and seller-aware cart/order/fulfillment grouping without seller portal, payouts, commissions and disputes.
- Then we move to Pricing 2.0: channel-aware price resolution, price lists, rules and promotions.
- After that we extract tax, post-order flows and provider SPI.

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## FFA core/transport/ui slice

Slice 10.6 fixes the structural shape `core_transport_ui`: admin and storefront received framework-agnostic `core.rs` helpers, module-owned `transport.rs` facades and explicit Leptos render adapters `admin/src/ui/leptos.rs` / `storefront/src/ui/leptos.rs`. Crate roots now only connect module layers and re-export `CommerceAdmin` / `CommerceView`; covered flows go through the transport facade rather than raw `api::*` functions.
