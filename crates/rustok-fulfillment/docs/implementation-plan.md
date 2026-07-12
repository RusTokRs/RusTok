# Implementation plan for `rustok-fulfillment`

## Current state

`rustok-fulfillment` owns shipping options, fulfillments, typed fulfillment
items, shipping-selection policy, and provider SPI policy. The commerce module
composes delivery groups, checkout, and multi-fulfillment orchestration; it
must not duplicate fulfillment transport, selection materialization, or carrier
lifecycle persistence.

The owner storefront handles seller-aware shipping selection through native and
GraphQL transports. Selection identity is exactly `shipping_profile_slug +
seller_id`; legacy `seller_scope` is not accepted. Provider registry guards
capability, health, unavailable mode, and degraded fallback before an adapter
call, while `FulfillmentService` remains the lifecycle owner.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `fulfillment.shipping_selection.v1` in
  `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`.
- Contract and provider evidence:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-contract-test-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json`,
  and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`.
- `scripts/verify/verify-fulfillment-admin-boundary.mjs` and
  `scripts/verify/verify-fulfillment-storefront-boundary.mjs` lock the
  owner-admin/storefront split and host-neutral native shipping-selection path.
- Storefront owner transport uses `execute_selected_transport` with native
  `#[server]` selected first and GraphQL retained as the parallel fallback.

## Open results

1. **Prove mixed-cart and multi-fulfillment edge cases.** Cover seller-aware
   selection, partial shipment/delivery, reopen/reship recovery, remaining
   quantity, and grouped checkout interactions without moving order or payment
   transitions into this module.
   **Depends on:** order-line and commerce delivery-group contracts.
   **Done when:** targeted tests cover valid and rejected transitions for a
   mixed cart and multiple fulfillment records.

2. **Wire production carrier adapters through the provider registry.** Add
   concrete carrier configuration, quote, label, cancellation, and replay-safe
   tracking-webhook execution only through guarded provider seams.
   **Depends on:** approved carrier credentials, webhook ingress, and
   deployment-owned secret management.
   **Done when:** production-like execution proves degraded fallback and typed
   adapter errors while `FulfillmentService` remains the sole lifecycle owner.

3. **Execute remote shipping-selection contract evidence.** Turn the locked
   in-process/remote matrix into provider execution before promoting beyond
   `boundary_ready`, keeping native and GraphQL storefront behavior aligned.
   **Depends on:** a remote adapter environment and a commerce consumer.
   **Done when:** deadline, idempotency, typed-error, seller-identity, and
   fallback parity are proven for `fulfillment.shipping_selection.v1`.

## Verification

- `npm run verify:fulfillment:admin-boundary`
- `npm run verify:fulfillment:storefront-boundary`
- `npm run verify:ecommerce:fba`
- `npm run verify:ecommerce:provider-spi-evidence`
- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- Targeted shipping-option, fulfillment-item, delivery-group, and
  multi-fulfillment tests.

## Change rules

1. Keep shipping selection, fulfillment lifecycle, and carrier policy here.
2. Update local documentation, `rustok-module.toml`, and the umbrella commerce
   plan with a delivery or provider contract change.
3. Update this status block and `docs/modules/registry.md` with any FFA/FBA
   boundary change.
