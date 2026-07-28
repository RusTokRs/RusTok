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

The root in-process checkout factory now mounts
`TypedCheckoutFulfillmentExecutionPort`. Ensure and recovery reads accept
`Pending`, `Shipped`, and `Delivered` owner states. `Cancelled` and unknown
lifecycle values fail closed with a typed manual-reconciliation outcome instead
of being adopted as a successful checkout fulfillment set. The underlying
execution adapter remains the persistence/idempotency delegate, so transport
contracts and request DTOs are unchanged.

Complete shipping-option active list and lookup use `ShippingOptionReadPort`,
while administrative list-all uses the separate `ShippingOptionAdminReadPort`.
The root in-process factories own `FulfillmentService` construction, require read
policy, preserve requested and default locale values, and map owner failures to
stable `PortError` envelopes. The application host now composes both read ports
once in `HostRuntimeContext`; manifest GraphQL runtime data carries them into a
resolver-scoped async task, and the private Commerce facade only consumes those
scoped owner ports. Mounted shipping enrichment, storefront listing, single
lookup, and administrative list-all therefore no longer construct their
in-process providers inside the Commerce seam. The facade retains one concrete
`FulfillmentService` only for fulfillment lifecycle and order-to-fulfillment
compatibility reads. Directly embedded standalone schemas retain an explicit
in-process compatibility fallback outside the facade. The seller/cart
`ShippingSelectionPort` contract is unchanged.

A source-only transport inventory now records that Commerce REST still constructs
`FulfillmentService` for storefront active-list and admin list-all/lookup, even
though it preserves the same successful filtering semantics. The native FFA
surface owns seller/cart selection and does not publish complete projection list
or lookup operations. The retained next implementation decision is REST cutover
to the host-composed owner ports without expanding `ShippingSelectionPort`.

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
- Source-ready internal read boundaries: `ShippingOptionReadPort` and
  `ShippingOptionAdminReadPort`; these are not new FBA provider contracts and do
  not change registry status.
- Contract and provider evidence:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-contract-test-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`,
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json`,
  and `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`.
- Source-only shipping-option transport inventory:
  `crates/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-source.json`.
  It is unvalidated source evidence and does not promote any status.
- `scripts/verify/verify-fulfillment-admin-boundary.mjs`,
  `scripts/verify/verify-fulfillment-storefront-boundary.mjs`,
  `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`, and
  `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs` lock the
  owner-admin/storefront, checkout execution, and typed lifecycle split.
- `scripts/verify/verify-fulfillment-shipping-option-read-port.mjs`,
  `scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs`, and
  `scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs`
  guard the internal read boundaries, mounted GraphQL host composition, and the
  explicit REST/native source inventory.
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

- [x] Publish active list and lookup through `ShippingOptionReadPort`.
- [x] Publish administrative list-all through the separate
  `ShippingOptionAdminReadPort`.
- [x] Keep active storefront listing and complete administrative listing as
  separate traits rather than an overloaded request flag or a growing storefront
  interface.
- [x] Preserve requested and tenant-default locale arguments.
- [x] Require read policy and parse tenant identity from `PortContext`.
- [x] Map all current `FulfillmentError` variants to stable `PortError` values.
- [x] Export canonical root in-process factories.
- [x] Remove direct `FulfillmentService` construction from mounted commerce
  GraphQL shipping-option validation, enrichment, and storefront listing.
- [x] Cut mounted GraphQL single lookup and administrative `shipping_options`
  list-all over to owner read ports while preserving optional-not-found and
  `FULFILLMENT_*` public envelopes.
- [x] Inject both read ports from application-host composition through typed
  runtime data and resolver-scoped async context; keep standalone fallback outside
  the private facade.
- [x] Retain a source-only GraphQL/REST/native inventory that distinguishes
  complete projection reads from seller/cart selection and records the REST
  cutover decision without claiming runtime parity.
- [ ] Cut Commerce REST storefront active-list and admin list-all/lookup over to
  the same host-composed owner runtime while preserving HTTP envelopes and local
  success filters.
- [ ] Execute compile, mounted GraphQL/REST parity, deadline, locale, channel,
  failure, and remote-profile evidence.

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

5. **Cut REST projection reads over and prove transport parity.** Replace the
   existing Commerce REST storefront active-list and admin list-all/lookup
   concrete service construction with `CommerceShippingOptionReadRuntime`, then
   execute mounted GraphQL/REST comparisons against the same owner projections.
   Native seller/cart selection remains a separate contract and needs no complete
   projection surface without a consumer.
   **Depends on:** host runtime wiring for Commerce HTTP plus compiled
   fulfillment/commerce crates and mounted transport fixtures.
   **Done when:** GraphQL and REST retain locale/channel/deadline context, expose
   identical owner projections with correct active/inactive semantics, preserve
   their public envelopes, and no mounted projection transport constructs a
   concrete fulfillment service or in-process read provider.

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
- `node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs`
- `node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs`
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
  context, owner-error, GraphQL/REST parity, and remote-profile tests.

No verification command was executed in this source wave.

## Change rules

1. Keep shipping selection, shipping-option projections, fulfillment lifecycle,
   checkout fulfillment identity, and carrier policy here.
2. Update local documentation, contracts, `rustok-module.toml`, and the umbrella
   commerce plan with a delivery or provider contract change.
3. Update this status block and `docs/modules/registry.md` only with proven
   FFA/FBA boundary changes.
