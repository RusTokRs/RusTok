# Marketplace family implementation plan

Last reviewed: 2026-07-17

## Status

- FFA status: `not_started`.
- FBA status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- Family source gate: `open`.
- Production promotion gate: `closed`.

Source implementation does not promote a module without retained compile,
migration, contention, mounted-transport, and remote-profile evidence.

## FBA/FFA architecture contract

- [x] Use the mandatory `rustok-marketplace-*` crate family and `marketplace_*`
  module slugs.
- [x] Keep the family root as composition/orchestration only; it owns no seller,
  listing, allocation, commission, ledger, or payout tables.
- [x] Keep owner persistence, lifecycle transitions, receipts, events, and provider
  policy inside the corresponding owner module.
- [x] Communicate through typed FBA ports carrying tenant, actor, effective locale,
  channel, correlation, deadline, and idempotency context.
- [x] Keep host applications and FFA packages free of owner policy and owner entity
  imports.
- [x] Use the structural sequence `core_only -> core_transport ->
  core_transport_ui`.
- [x] Require explicit native/GraphQL transport selection; an FFA adapter must not
  silently fall back to another transport.
- [ ] Retain compiled in-process and remote-profile contract evidence before FBA
  `transport_verified`.
- [ ] Retain mounted native/GraphQL workflow parity before FFA `phase_b_ready`.

## Completed family source work

- [x] Publish `rustok-marketplace` as the Marketplace Family root.
- [x] Publish root-owned seller directory composition over
  `MarketplaceSellerReadPort`.
- [x] Publish root-owned listing directory and eligibility composition over
  `MarketplaceListingReadPort`.
- [x] Keep root consumers free of SeaORM, foreign entities, and owner database
  access.
- [x] Publish `rustok-marketplace-seller` as the seller identity, membership,
  onboarding, lifecycle, and localized profile owner.
- [x] Publish `rustok-marketplace-listing` as the seller listing, versioned terms,
  lifecycle, moderation-event, and eligibility owner.
- [x] Register seller, listing, and root modules as opt-in composition features;
  Marketplace is not default-enabled.
- [x] Add family, seller transport, listing boundary, and listing lifecycle-event
  source guards.

## Owner boundaries

### Seller

- [x] Own language-agnostic seller rows and normalized
  `marketplace_seller_translations`.
- [x] Use exact effective locale supplied by `PortContext`; owner-side fallback is
  forbidden.
- [x] Persist durable command receipts atomically with seller writes and localized
  result snapshots.
- [x] Publish module-owned native/GraphQL FFA source workflows.
- [ ] Replace mutable onboarding/suspension prose with immutable locale-tagged seller
  lifecycle/moderation events.
- [ ] Add normalized verification facts and a KYC provider SPI without raw provider
  payload persistence.
- [ ] Retain compiled/mounted FBA and FFA evidence.

### Listing

- [x] Own seller/master-variant/market/channel listing identity and versioned terms.
- [x] Keep product title, description, translations, price, stock, and fulfillment
  state in their owner modules.
- [x] Store no localized product copy or locale-keyed JSON maps in listing tables.
- [x] Persist durable command receipts and replay admission.
- [x] Persist append-only locale-tagged listing events with actor and bounded timeline
  reads.
- [x] Route terms update, submit, review, suspend, and archive through atomic
  state/terms + event + receipt transactions.
- [ ] Add immutable events to create, publish, and reactivate while preserving
  provider-preflight replay.
- [ ] Backfill events and remove mutable `approval_note` and `suspension_reason`
  compatibility columns.
- [ ] Add module-owned listing FFA package and native/GraphQL transports.
- [ ] Retain compiled/mounted FBA and FFA evidence.

### Future owners

- [ ] Add durable seller order allocations without duplicating the customer order
  aggregate.
- [ ] Add `rustok-marketplace-commission` with versioned policy snapshots and
  deterministic explanations.
- [ ] Add immutable double-entry `rustok-marketplace-ledger` before balances or
  payouts.
- [ ] Add `rustok-marketplace-payout` with idempotent journals, provider SPI,
  reconciliation, reversals, and operator audit.

## FBA promotion

- [ ] Reach family `boundary_ready` after seller/listing compiled contracts,
  migrations, receipt/event contention tests, root consumer execution, and source
  guards are retained.
- [ ] Reach family `transport_verified` only after in-process and remote-profile
  timeout, degraded-mode, fallback, and mounted consumer evidence is retained.

## FFA promotion

The root currently owns no UI. Seller, listing, commission, ledger, and payout UI
must be published by their owner modules. A future aggregate Marketplace control
room may only compose owner view models and transport facades.

- [ ] Seller FFA: retain mounted native/GraphQL parity and localized error evidence.
- [ ] Listing FFA: publish module-owned core/model/transport/i18n/Leptos package and
  retain mounted native/GraphQL parity.
- [ ] Keep vendor portal, platform admin, and storefront hosts as composition shells;
  they must not implement marketplace policy.

## Immediate execution order

1. [ ] Complete immutable listing events for create, publish, and reactivate.
2. [ ] Backfill listing compatibility snapshots and remove mutable note columns.
3. [ ] Complete immutable seller lifecycle/moderation events.
4. [ ] Add listing FFA package and explicit native/GraphQL transports.
5. [ ] Compile seller/listing/root contracts and apply SQLite/PostgreSQL migrations.
6. [ ] Execute idempotency, locale, tenant-isolation, contention, restart, and mounted
   transport scenarios.
7. [ ] Start seller order allocation, then commission, ledger, and payout owners.

## Source evidence

- `src/seller_directory.rs`
- `src/listing_directory.rs`
- `contracts/marketplace-fba-registry.json`
- `../rustok-marketplace-seller/docs/implementation-plan.md`
- `../rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json`
- `../rustok-marketplace-listing/docs/implementation-plan.md`
- `../rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json`
- `../../apps/server/tests/marketplace_family_boundary_guard.rs`
- `../../apps/server/tests/marketplace_seller_transport_guard.rs`
- `../../apps/server/tests/marketplace_listing_boundary_guard.rs`
- `../../apps/server/tests/marketplace_listing_lifecycle_event_guard.rs`
- `../../scripts/verify/verify-marketplace-family-boundary.mjs`
- `../../scripts/verify/verify-marketplace-seller-transport.mjs`
- `../../scripts/verify/verify-marketplace-listing-boundary.mjs`
- `../../scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`
