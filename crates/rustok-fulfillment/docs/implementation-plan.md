# Implementation plan for `rustok-fulfillment`

Last reviewed: 2026-07-28

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

Checkout fulfillment create/adopt/read enters through
`CheckoutFulfillmentExecutionPort`. Commerce sends typed order-line commands
derived from the immutable checkout plan and receives normalized fulfillment
projections. Fulfillment owner uses `FulfillmentService::list_by_order` and
`create_fulfillment`; mounted commerce checkout no longer queries the
`fulfillments` table or constructs `FulfillmentService`.

Complete shipping-option active list, administrative list-all, and lookup now
have a separate read-only owner boundary, `ShippingOptionReadPort`. The root
in-process factory owns `FulfillmentService` construction, requires read policy,
preserves requested and default locale values, and maps owner failures to stable
`PortError` envelopes. Mounted commerce GraphQL shipping-option validation,
shipping enrichment, and storefront listing use this port instead of
constructing `FulfillmentService` directly. The administrative list-all owner
contract is source-ready for the remaining mounted GraphQL cutover. The
seller/cart `ShippingSelectionPort` contract is unchanged.

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
- Mounted in-process provider: `TypedCheckoutFulfillmentExecutionPort` over the
  existing owner execution adapter.
- Source-ready internal read boundary: `ShippingOptionReadPort`; this is not a
  new FBA provider contract and does not change registry status.
- Contract and provider evidence:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-contract-test-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json`,
  and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`.
- `scripts/verify/verify-fulfillment-admin-boundary.mjs`,
  `scripts/verify/verify-fulfillment-storefront-boundary.mjs`,
  `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`, and
  `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs` lock the
  owner-admin/storefront, checkout execution, and typed lifecycle split.
- `scripts/verify/verify-fulfillment-shipping-option-read-port.mjs` guards the
  internal shipping-option read boundary and mounted storefront commerce
  cutover.
- No status promotion is claimed from source. Compile, upgraded database,
  contention, restart, mounted transport, and remote evidence remain missing.

## Checkout execution source checklist

- [x] Publish typed create/adopt/read commands through
  `CheckoutFulfillmentExecutionPort`.
- [x] Keep fulfillment persistence and metadata compatibility lookup inside the
  owner module.
- [x] Mount the root in-process factory through typed lifecycle validation.
- [x] Accept pending, shipped, and delivered replay projections.
- [x] Route cancelled and unknown checkout fulfillment lifecycle states to
  manual reconciliation.
- [x] Guard the mounted commerce path against direct construction of the legacy
  in-process execution adapter.
- [ ] Replace metadata identity with owner-owned typed persistence and a
  concurrency-safe uniqueness constraint.
- [ ] Execute compile, create/adopt/read, duplicate identity, lifecycle,
  process-exit, restart, contention, and remote-profile evidence.

## Shipping-option read source checklist

- [x] Publish active list, administrative list-all, and lookup operations through
  `ShippingOptionReadPort`.
- [x] Keep active storefront listing and complete administrative listing as
  explicit operations rather than an overloaded request flag.
- [x] Preserve requested and tenant-default locale arguments.
- [x] Require read policy and parse tenant identity from `PortContext`.
- [x] Map all current `FulfillmentError` variants to stable `PortError` values.
- [x] Export a canonical root in-process factory.
- [x] Remove direct `FulfillmentService` construction from mounted commerce
  GraphQL shipping-option validation, enrichment, and storefront listing.
- [ ] Cut the mounted administrative GraphQL `shipping_options` query over to
  `list_all_shipping_option_projections`.
- [ ] Inject the read port from the application host rather than constructing the
  root in-process provider inside the commerce seam.
- [ ] Execute compile, mounted GraphQL, REST/native parity, deadline, failure,
  and remote-profile evidence.

## Open results

1. **Prove checkout fulfillment identity and replay.** Execute create/adopt/read,
   duplicate key, partial set, concurrent create, process-exit, restart, and
   upgraded metadata scenarios through the mounted commerce stage.
   **Depends on:** compiled commerce/fulfillment crates and migrated databases.
   **Done when:** one immutable plan produces one exact fulfillment set and every
   conflicting, cancelled, unknown, or duplicate identity fails closed.

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

5. **Prove and host-compose shipping-option reads.** Cut the mounted
   administrative list-all query over to the owner port, execute active list,
   administrative list-all, and lookup through mounted GraphQL consumers,
   compare REST/native behavior, and move provider construction to the
   application host.
   **Depends on:** compiled fulfillment/commerce crates and mounted transport
   fixtures.
   **Done when:** all transports retain locale/channel/deadline context, expose
   identical owner projections with correct active/inactive semantics, and no
   commerce transport constructs a concrete fulfillment service or in-process
   provider.

6. **Execute remote contracts.** Turn shipping-selection and checkout-execution
   matrices into provider execution before promoting beyond `boundary_ready`.
   **Depends on:** a remote adapter environment and a commerce consumer.
   **Done when:** deadline, idempotency, typed-error, identity, and fallback
   parity are proven.

## Verification

- `npm run verify:fulfillment:admin-boundary`
- `npm run verify:fulfillment:storefront-boundary`
- `node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- `node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`
- `node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs`
- `node scripts/verify/verify-commerce-graphql-shipping-option-typed-error.mjs`
- `node scripts/verify/verify-commerce-graphql-shipping-enrichment-typed-error.mjs`
- `npm run verify:ecommerce:fba`
- `npm run verify:ecommerce:provider-spi-evidence`
- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- `cargo check -p rustok-fulfillment --all-features`
- `cargo check -p rustok-commerce --all-features`
- Targeted checkout fulfillment create/adopt/read, cancelled/unknown lifecycle,
  duplicate identity, process-exit, restart, and multi-fulfillment tests.
- Targeted shipping-option active list, administrative list-all, lookup locale,
  context, owner-error, GraphQL, REST/native parity, and remote-profile tests.

No verification command was executed in this source wave.

## Change rules

1. Keep shipping selection, shipping-option projections, fulfillment lifecycle,
   checkout fulfillment identity, and carrier policy here.
2. Update local documentation, contracts, `rustok-module.toml`, and the umbrella
   commerce plan with a delivery or provider contract change.
3. Update this status block and `docs/modules/registry.md` only with proven
   FFA/FBA boundary changes.
