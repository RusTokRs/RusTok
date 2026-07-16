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
- FBA status: `boundary_ready` (`core_transport_ui`); source inspection and
  committed recovery code do not promote the boundary without live execution.
- Structural shape: `core_transport_ui`.
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
- `CheckoutOperationJournal` persists the operation identity, request and cart
  snapshot hashes, execution lease, recovery status, stage checkpoints, and
  owner aggregate ids. `CheckoutInventoryReservationJournal` adds one immutable
  reservation identity per checkout/cart-line pair and enforces tenant, cart,
  variant, quantity, location, status-transition, and timestamp integrity in
  PostgreSQL and SQLite.
- Storefront checkout now requires a stable `Idempotency-Key` and uses
  `StagedCheckoutService` rather than the former monolithic
  `CheckoutService::complete_checkout` production entrypoint.
- `CheckoutPlanBuilder` validates product, inventory, shipping, channel, locale,
  and store context from the prepared cart and produces one immutable order and
  fulfillment plan before cross-domain side effects begin.
- `CheckoutStagePipeline` resumes through `cart_locked`,
  `inventory_reserved`, `order_created`, `payment_ready`,
  `payment_authorized`, `payment_captured`, `fulfillment_created`,
  `cart_completed`, and `completed`.
- `CheckoutInventoryReservationExecutor` invokes the inventory-owned
  `InventoryReservationIdentityPort` from the immutable prepared cart snapshot,
  adopts existing owner reservations on replay, records provider failures, and
  checkpoints `inventory_reserved` only after every variant-backed line is
  confirmed. Order creation adopts those identities into order lines, while
  the lifecycle cutover prevents the legacy confirmation trigger from reserving
  the same demand a second time. The executor is not yet composed into
  `JournaledCheckoutService` because the current order-confirmation trigger
  also reserves inventory, and enabling both paths would double-reserve demand.
- Order, payment collection, provider operation, fulfillment, and cart
  completion stages use durable owner identities and accept already committed
  results on replay.
- `CheckoutCompensationService` separates pre-order identity release from
  post-order cancellation. Adopted stock is released through the order
  lifecycle trigger; only remaining pre-adoption identities are released
  directly. Captured or uncertain provider outcomes are not automatically
  reversed and remain reconciliation work.
- `RecoveringStagedCheckoutService` attempts safe compensation immediately
  after a storefront failure. `CheckoutCompensationSweepService` provides a
  bounded lease-protected retry path for compensation backlog and expired
  compensation workers.
- `PaymentProviderOperationJournal::list_by_collection` gives orchestration an
  owner-supported read of provider side effects before automatic cancellation.
- REST storefront cart handlers plus GraphQL cart reads and mutations call
  cart-owned `CartStorefrontPort` for cart reads, creation, line-item
  mutations, context updates, and repricing; REST, GraphQL, and native
  checkout adapters use the same owner ports for their cart boundary.
- GraphQL and native admin cart promotions call cart-owned `CartPromotionPort`; no
  production `rustok-commerce` adapter constructs `CartService` directly.
- Durable checkout pricing resolves variants through pricing-owned
  `PricingReadPort`; the checkout snapshot resolver no longer constructs
  `PricingService` directly.
- REST, GraphQL, and native storefront cart repricing, plus REST and GraphQL
  add-to-cart and line-item quantity resolution, consume `PricingReadPort`
  with typed context and no direct variant-price bypass.
- GraphQL admin/storefront pricing-product roots resolve effective variant prices
  through `PricingReadPort`; the typed not-found result preserves the existing
  nullable effective-price response rather than failing the whole projection.
- The storefront active-price-list GraphQL root consumes the pricing-owned list
  projection port instead of constructing `PricingService` directly.
- The admin pricing-product GraphQL root consumes the owner projection port and
  carries the authenticated actor plus request-derived locale/channel context.
- The storefront pricing-product-by-handle GraphQL root also consumes the owner
  projection port with its public channel scope rather than constructing a
  pricing service.
- GraphQL admin pricing writes consume `PricingWritePort` for variant-price upsert,
  discount application, active price-list rule changes, and scope changes; the
  commerce adapter does not construct `PricingService` or perform a post-write
  owner lookup directly.
- Targeted compiled provider-consumer execution is recorded by
  `cargo test -p rustok-commerce --test checkout_service_test
  validation::complete_checkout_rejects_line_item_without_channel_visible_inventory -- --exact`.
  It executes the real cart, product, and inventory providers through checkout
  and proves that channel-hidden inventory blocks checkout. It does not cover
  the remaining checkout providers or fallback/degraded paths, so commerce
  remains `in_progress`.
- No new staged-checkout code has live compile, migration, concurrency, or
  kill-point evidence yet. FFA/FBA status therefore remains `in_progress`.
- FFA guardrails: `scripts/verify/verify-commerce-admin-boundary.mjs` locks
  `admin/src/transport/native_server_adapter.rs`, removed root GraphQL and state-machine aliases,
  and the core/transport/UI owner boundary;
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` protects
  the checkout aggregate handoff through
  `storefront/src/transport/native_server_adapter.rs`.

## Next results

1. **Obtain live staged-checkout evidence.** Compile the commerce family,
   execute clean and upgraded PostgreSQL/SQLite migrations, and add kill points
   after every owner call and before every checkpoint. Done when the same
   operation resumes without duplicate reservations, orders, collections,
   provider operations, fulfillments, labels, or cart completion.
2. **Close compensation lookup and operator surfaces.** Recover a payment
   collection by checkout identity even when its id was not checkpointed, expose
   bounded admin compensation/reconciliation commands, and record safe error
   codes without leaking provider or SQL details. Done when every
   `compensation_required` operation is either compensated, retryable with a
   lease, or explicitly classified for manual reconciliation.
3. **Complete provider and transport evidence.** Execute product, pricing,
   inventory, cart, payment, fulfillment, and region ports with real deadlines,
   retry, degraded, malformed-response, and unavailable cases. Done when
   native/REST/GraphQL behavior is consistent and evidence is generated from
   execution rather than source markers.
4. **Productionize payment and fulfillment providers.** Wire approved payment
   gateways and carrier adapters through owner registries, then prove
   authorize/capture/cancel/refund, quote/label/cancel, webhooks, retries, and
   reconciliation. Done when the manual provider is no longer the only
   executable production profile.
5. **Deliver the next ecommerce increments by owner.** Continue Pricing 2.0,
   promotion, market/store, full post-order, seller/offer, commission, ledger,
   and payout capabilities only after the staged checkout correctness gate is
   backed by live evidence.

## Verification

- `cargo fmt --all -- --check`
- `cargo check -p rustok-commerce --lib`
- `cargo test -p rustok-commerce --test checkout_service_test`
- `cargo test -p rustok-commerce --test order_inventory_reservation_test`
- `cargo test -p rustok-migrations --test ecommerce_schema_smoke`
- `npm run verify:ecommerce:fba`
- `npm run verify:commerce:admin-boundary`
- `npm run verify:commerce:storefront-transport-handoff`
- `cargo xtask module validate commerce`
- PostgreSQL contention and process kill-point tests for every checkout stage.

## References

- [Crate README](../README.md)
- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
