# rustok-commerce implementation plan

## Current state

`rustok-commerce` is the ecommerce orchestration root, not a replacement for
split bounded contexts. It owns checkout, cross-domain context, shipping
profiles, aggregate cart promotion and remaining post-order orchestration. The
product, cart, customer, region, pricing, inventory, order, payment, and
fulfillment modules own their services, persistence, and domain UI surfaces.
The aggregate storefront now focuses on checkout; payment/refund projection is
already handed to payment-owned storefront transport.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `in_progress` (`core_transport_ui`); consumer metadata and
  source-smoke evidence do not promote the boundary without live execution.
- Structural shape: `core_transport_ui`
- The commerce consumer registry is
  `crates/rustok-commerce/contracts/commerce-fba-registry.json`; its runtime
  invocation trace is
  `crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json`.
- Provider contracts remain owner-owned: `crates/rustok-pricing/contracts/pricing-fba-registry.json`,
  `crates/rustok-inventory/contracts/inventory-fba-registry.json`,
  `crates/rustok-order/contracts/order-fba-registry.json`,
  `crates/rustok-payment/contracts/payment-fba-registry.json`,
  `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`,
  `crates/rustok-product/contracts/product-fba-registry.json`,
  `crates/rustok-customer/contracts/customer-fba-registry.json`, and
  `crates/rustok-cart/contracts/cart-fba-registry.json`.
- The bounded checkout → inventory availability seam now calls the injected
  `InventoryReservationPort` with tenant, user actor, normalized cart locale,
  channel, correlation id, and a read deadline. It maps provider failures to
  `CheckoutError::BoundaryFailure` and no longer uses the inventory public
  helper directly. The status remains `in_progress`: the targeted
  `complete_checkout_rejects_line_item_without_channel_visible_inventory`
  integration test still needs a completed local or CI execution record before
  this provider-consumer seam can be counted as live evidence.
- Checkout now also reads product and variant-first catalog projections through
  `ProductCatalogReadPort`. The product owner resolves a variant id to its
  product projection, while checkout keeps cart-specific channel and shipping
  snapshot validation. Checkout no longer imports product entities. This
  additive owner operation has static contract evidence only; the FBA status
  remains `in_progress` until provider-consumer execution is recorded.
- Checkout now calls cart-owned `CartCheckoutPort` for snapshot reads, context
  updates, and `begin/release/complete` lifecycle transitions. Every write has
  a checkout-derived idempotency key and deadline; direct `CartService` use is
  removed from checkout orchestration. Runtime evidence remains required.
- FFA guardrails: `scripts/verify/verify-commerce-admin-boundary.mjs` locks
  `admin/src/transport/native_server_adapter.rs`, removed root GraphQL and state-machine aliases,
  and the core/transport/UI owner boundary;
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` protects
  the checkout aggregate handoff through
  `storefront/src/transport/native_server_adapter.rs`.

## Next results

1. **Complete checkout owner handoff with live boundary evidence.** Reduce the
   aggregate cart projection only as a whole owner-handoff package, preserve
   checkout orchestration, and execute product/pricing/inventory/customer/cart
   provider fallbacks under real runtime conditions. Done when an end-to-end
   checkout verifies request context, degraded behavior, recovery, and
   native/GraphQL parity without owner service or DTO re-exports returning to
   commerce.
2. **Productionize payment and fulfillment providers.** Wire approved payment
   gateways and carrier adapters through owner `PaymentProviderRegistry` and
   `FulfillmentProviderRegistry`, then prove authorize/capture/cancel/refund,
   quote/label/cancel, webhooks, retries, and recovery. Done when the manual
   default is no longer the only executable provider and lifecycle persistence
   remains in owner services.
3. **Deliver the next ecommerce domain increments by owner.** Extend the
   seller foundation only through stable seller/catalog/grouping contracts;
   deliver channel-aware Pricing 2.0, separate tax calculation, and the
   remaining exchange/claim/order-change post-order surface in their owning
   modules. Done when each increment has a named owner, ports/events, FFA
   parity, FBA evidence, and no host or umbrella-domain regression.

## Verification

- `npm run verify:ecommerce:fba`
- `npm run verify:commerce:admin-boundary`
- `npm run verify:commerce:storefront-transport-handoff`
- `cargo xtask module validate commerce`
- Targeted checkout, provider-adapter, post-order, inventory-channel, and
  owner-handoff integration tests.

## References

- [Crate README](../README.md)
- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
