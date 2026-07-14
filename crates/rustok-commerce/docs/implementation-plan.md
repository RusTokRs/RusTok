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
- Storefront checkout now requires a stable `Idempotency-Key` and uses
  `StagedCheckoutService` rather than the former monolithic
  `CheckoutService::complete_checkout` production entrypoint.
- `CheckoutOperationJournal` persists request and snapshot hashes, execution
  leases, recovery status, stage checkpoints, and owner aggregate ids.
- `CheckoutPlanBuilder` validates product, inventory, shipping, channel, locale,
  and store context from the prepared cart and produces one immutable order and
  fulfillment plan before cross-domain side effects begin.
- `CheckoutStagePipeline` resumes through `cart_locked`,
  `inventory_reserved`, `order_created`, `payment_ready`,
  `payment_authorized`, `payment_captured`, `fulfillment_created`,
  `cart_completed`, and `completed`.
- Checkout inventory reservation identity is persisted before the inventory
  owner call. Order creation adopts those identities into order lines, while
  the lifecycle cutover prevents the legacy confirmation trigger from reserving
  the same demand a second time.
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
- REST storefront cart handlers plus GraphQL cart reads and mutations continue
  to call cart-owned ports. Payment and fulfillment lifecycle persistence
  remains owner-owned.
- No new code in this cutover has live compile, migration, concurrency, or
  kill-point evidence yet. FFA/FBA status therefore remains `in_progress`.

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
