# Marketplace listing implementation plan

Last reviewed: 2026-07-16

## Status

- FFA status: `not_started`.
- FBA status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- Owner source gate: `open`.
- Production promotion gate: `closed`.

## Ownership

- [x] Own one listing aggregate per seller/master-variant/market/channel scope.
- [x] Keep seller SKU uniqueness inside seller scope.
- [x] Store immutable versioned commercial references in
  `marketplace_listing_terms`.
- [x] Keep canonical product content, prices, stock, and fulfillment state in their
  owner modules.
- [x] Use no cross-module database foreign keys.
- [x] Own draft, review, publish, suspend, reactivate, archive, and terms-version
  transitions.
- [x] Persist tenant-scoped command receipts atomically with owner writes and
  normalized response snapshots.
- [x] Publish eligibility reason codes without implementing buy-box ranking.

## FBA

- [x] Publish `MarketplaceListingReadPort` for detail, directory, and eligibility.
- [x] Publish `MarketplaceListingCommandPort` for create, terms update, review,
  publication, suspension/reactivation, and archive.
- [x] Resolve seller identity/status through `MarketplaceSellerReadPort`.
- [x] Resolve variant-to-master-product identity through `ProductCatalogReadPort`.
- [x] Require deadlines for reads and deadline plus idempotency key for writes.
- [x] Publish stable safe `PortError` mappings without SQL/driver details.
- [ ] Add root Marketplace consumer composition and source guards.
- [ ] Compile owner/provider contracts.
- [ ] Apply clean/upgraded SQLite and PostgreSQL migrations.
- [ ] Execute receipt replay, conflicting payload, same-key contention, scope/SKU
  conflicts, terms version races, and lifecycle contention.
- [ ] Retain remote-profile timeout/degraded/fallback evidence before promotion.

## FFA

- [ ] Add `rustok-marketplace-listing-admin` with framework-neutral models/core,
  explicit native/GraphQL transport selection, i18n, and Leptos adapter.
- [ ] Add listing directory/detail, terms history, review, publication, suspension,
  and eligibility explanation workflows.
- [ ] Add platform `marketplace_listings` permissions without moving seller/listing
  policy into host code.
- [ ] Retain mounted native/GraphQL parity evidence before `phase_b_ready`.

## Source evidence

- `src/entities/listing.rs`
- `src/entities/listing_terms.rs`
- `src/entities/listing_command_receipt.rs`
- `src/migrations/m20260716_000001_create_marketplace_listings.rs`
- `src/command_receipts.rs`
- `src/service.rs`
- `src/ports.rs`
- `contracts/marketplace-listing-fba-registry.json`

## Promotion gates

- [ ] FBA `boundary_ready`: source guards plus compiled owner/provider-consumer
  contracts and durable identity tests are retained.
- [ ] FBA `transport_verified`: mounted in-process/remote execution and degraded
  behavior are retained.
- [ ] FFA `phase_b_ready`: module-owned admin package, host composition, and
  native/GraphQL parity evidence are retained.
- [ ] Production-ready: migrations, tenant isolation, contention, restart, and
  storefront selection integration are retained.
