# Marketplace listing implementation plan

Last reviewed: 2026-07-17

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
- [x] Keep canonical product title, description, translations, prices, stock, and
  fulfillment state in their owner modules.
- [x] Store no localized product copy or locale-keyed JSON maps in listing or terms
  rows.
- [x] Keep seller SKU, market/channel scope, pricing/inventory references, and
  fulfillment profile language-agnostic.
- [x] Use no cross-module database foreign keys.
- [x] Own draft, review, publish, suspend, reactivate, archive, and terms-version
  transitions.
- [x] Persist tenant-scoped command receipts atomically with owner writes and
  normalized response snapshots.
- [x] Publish eligibility reason codes without implementing buy-box ranking.
- [ ] Replace mutable `approval_note` and `suspension_reason` base-row fields with
  immutable `marketplace_listing_events` carrying actor, effective locale, event
  kind, normalized note, metadata, and timestamp.
- [ ] Publish listing lifecycle/moderation events through the transactional outbox.

## FBA

- [x] Publish `MarketplaceListingReadPort` for detail, directory, and eligibility.
- [x] Publish `MarketplaceListingCommandPort` for create, terms update, review,
  publication, suspension/reactivation, and archive.
- [x] Resolve seller identity/status through `MarketplaceSellerReadPort`.
- [x] Resolve variant-to-master-product identity through `ProductCatalogReadPort`.
- [x] Require deadlines for reads and deadline plus idempotency key for writes.
- [x] Publish stable safe `PortError` mappings without SQL/driver details.
- [x] Compose root Marketplace listing directory/eligibility consumers without owner
  entities or database access.
- [x] Compile replay-safe command wrappers into the owner crate and route FBA create,
  publish, and reactivate commands through replay admission before seller/product
  provider reads.
- [x] Add source guards for schema ownership, absence of localized catalog copy,
  versioned terms, durable receipts, provider-preflight replay, deterministic reason
  codes, and root/distribution/server composition.
- [x] Register `marketplace_listing` in `modules.toml`, distribution, and server as an
  opt-in owner module; Marketplace remains excluded from default module sets.
- [ ] Include effective locale in request identity for review/suspend commands and
  persist immutable locale-tagged moderation events in the same receipt transaction.
- [ ] Compile owner/provider/root consumer contracts.
- [ ] Apply clean/upgraded SQLite and PostgreSQL migrations.
- [ ] Execute receipt replay, conflicting payload, same-key contention, scope/SKU
  conflicts, terms version races, lifecycle contention, and moderation-event
  atomicity.
- [ ] Retain remote-profile timeout/degraded/fallback evidence before promotion.

## FFA

- [ ] Add `rustok-marketplace-listing-admin` with framework-neutral models/core,
  explicit native/GraphQL transport selection, i18n, and Leptos adapter.
- [ ] Add listing directory/detail, terms history, moderation-event history, review,
  publication, suspension, and eligibility explanation workflows.
- [ ] Add platform `marketplace_listings` permissions without moving seller/listing
  policy into host code.
- [ ] Retain mounted native/GraphQL parity evidence before `phase_b_ready`.

## Source evidence

- `src/entities/listing.rs`
- `src/entities/listing_terms.rs`
- `src/entities/listing_command_receipt.rs`
- `src/migrations/m20260716_000001_create_marketplace_listings.rs`
- `src/command_receipts.rs`
- `src/replay_safe_commands.rs`
- `src/service.rs`
- `src/ports.rs`
- `contracts/marketplace-listing-fba-registry.json`
- `../rustok-marketplace/src/listing_directory.rs`
- `../rustok-distribution/Cargo.toml`
- `../rustok-distribution/src/lib.rs`
- `../../apps/server/Cargo.toml`
- `../../apps/server/tests/marketplace_listing_boundary_guard.rs`
- `../../scripts/verify/verify-marketplace-listing-boundary.mjs`

## Promotion gates

- [ ] FBA `boundary_ready`: source guards plus compiled owner/provider-consumer
  contracts, durable identity tests, and immutable moderation-event storage are
  retained.
- [ ] FBA `transport_verified`: mounted in-process/remote execution and degraded
  behavior are retained.
- [ ] FFA `phase_b_ready`: module-owned admin package, host composition, and
  native/GraphQL parity evidence are retained.
- [ ] Production-ready: migrations, tenant isolation, contention, restart, and
  storefront selection integration are retained.
