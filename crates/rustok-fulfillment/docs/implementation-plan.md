# Implementation plan for `rustok-fulfillment`

Last reviewed: 2026-07-22

## Current state

`rustok-fulfillment` owns shipping options, fulfillments, typed fulfillment
items, shipping-selection policy, and provider SPI policy. The commerce module
composes delivery groups, checkout, and multi-fulfillment orchestration; it
must not duplicate fulfillment transport, selection materialization, carrier
lifecycle persistence, or fulfillment recovery queries.

The owner storefront handles seller-aware shipping selection through native and
GraphQL transports. Selection identity is exactly `shipping_profile_slug +
seller_id`; legacy `seller_scope` is not accepted. Provider registry guards
capability, health, unavailable mode, and degraded fallback before an adapter
call, while `FulfillmentService` remains the lifecycle owner.

Checkout fulfillment create/adopt/read now enters through
`CheckoutFulfillmentExecutionPort`. Commerce sends typed order-line commands
derived from the immutable checkout plan and receives normalized fulfillment
projections. Fulfillment owner uses `FulfillmentService::list_by_order` and
`create_fulfillment`; mounted commerce checkout no longer queries the
`fulfillments` table or constructs `FulfillmentService`.

Stable fulfillment keys and metadata identity remain owner-local compatibility
mechanisms. Duplicate keys fail closed. A typed durable checkout fulfillment
identity and database uniqueness migration remain open.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `fulfillment.shipping_selection.v1` in
  `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`.
- Additional workflow contract: `fulfillment.checkout_execution.v1` in
  `crates/rustok-fulfillment/contracts/fulfillment-checkout-execution-v1.json`.
- Published checkout execution port: `CheckoutFulfillmentExecutionPort`.
- Contract and provider evidence:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-contract-test-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json`,
  and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`.
- `scripts/verify/verify-fulfillment-admin-boundary.mjs`,
  `scripts/verify/verify-fulfillment-storefront-boundary.mjs`, and
  `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs` lock the
  owner-admin/storefront and checkout execution split.
- No status promotion is claimed from source. Compile, upgraded database,
  contention, restart, mounted transport, and remote evidence remain missing.

## Open results

1. **Prove checkout fulfillment identity and replay.** Execute create/adopt/read,
   duplicate key, partial set, concurrent create, process-exit, restart, and
   upgraded metadata scenarios through the mounted commerce stage.
   **Depends on:** compiled commerce/fulfillment crates and migrated databases.
   **Done when:** one immutable plan produces one exact fulfillment set and every
   conflicting or duplicate identity fails closed.

2. **Replace metadata identity with typed persistence.** Add an owner-owned
   checkout fulfillment identity and uniqueness constraint without adding a
   foreign key to commerce-owned checkout tables.
   **Depends on:** retained upgraded compatibility evidence for current keys.
   **Done when:** recovery no longer scans metadata and concurrent creation cannot
   commit two rows for one checkout fulfillment index.

3. **Prove mixed-cart and multi-fulfillment edge cases.** Cover seller-aware
   selection, partial shipment/delivery, reopen/reship recovery, remaining
   quantity, and grouped checkout interactions without moving order or payment
   transitions into this module.
   **Depends on:** order-line and commerce delivery-group contracts.
   **Done when:** targeted tests cover valid and rejected transitions for a
   mixed cart and multiple fulfillment records.

4. **Wire production carrier adapters through the provider registry.** Add
   concrete carrier configuration, quote, label, cancellation, and replay-safe
   tracking-webhook execution only through guarded provider seams.
   **Depends on:** approved carrier credentials, webhook ingress, and
   deployment-owned secret management.
   **Done when:** production-like execution proves degraded fallback and typed
   adapter errors while `FulfillmentService` remains the sole lifecycle owner.

5. **Execute remote contracts.** Turn shipping-selection and checkout-execution
   matrices into provider execution before promoting beyond `boundary_ready`.
   **Depends on:** a remote adapter environment and a commerce consumer.
   **Done when:** deadline, idempotency, typed-error, identity, and fallback
   parity are proven.

## Verification

- `npm run verify:fulfillment:admin-boundary`
- `npm run verify:fulfillment:storefront-boundary`
- `node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- `npm run verify:ecommerce:fba`
- `npm run verify:ecommerce:provider-spi-evidence`
- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- `cargo check -p rustok-fulfillment --all-features`
- Targeted checkout fulfillment create/adopt/read, duplicate identity,
  process-exit, restart, and multi-fulfillment tests.

## Change rules

1. Keep shipping selection, fulfillment lifecycle, checkout fulfillment identity,
   and carrier policy here.
2. Update local documentation, contracts, `rustok-module.toml`, and the umbrella
   commerce plan with a delivery or provider contract change.
3. Update this status block and `docs/modules/registry.md` only with proven
   FFA/FBA boundary changes.
