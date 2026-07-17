# Marketplace listing implementation plan

Last reviewed: 2026-07-17

## Status

- FFA status: `not_started`.
- FBA status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- Owner source gate: `open`.
- Production promotion gate: `closed`.

Source implementation does not promote FBA or FFA without retained compile,
migration, contention, mounted-transport, and remote-profile evidence.

## FBA/FFA architecture contract

- [x] Keep listing identity, versioned terms, lifecycle, receipts, moderation events,
  and eligibility inside `rustok-marketplace-listing`.
- [x] Resolve seller and product facts through typed FBA ports; do not import foreign
  entities or add cross-module database foreign keys.
- [x] Keep product-owned localized title/description/translations outside listing
  storage.
- [x] Carry effective locale through `PortContext` for event-producing commands.
- [x] Keep the Marketplace root and future FFA hosts as composition surfaces only.
- [ ] Publish module-owned listing FFA core/model/transport/i18n/Leptos package before
  adding listing UI to hosts.
- [ ] Retain compiled remote-profile contract evidence before FBA
  `transport_verified`.
- [ ] Retain mounted native/GraphQL parity before FFA `phase_b_ready`.

## Ownership completed

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
- [x] Publish deterministic eligibility reason codes without implementing buy-box
  ranking.
- [x] Add append-only `marketplace_listing_events` with tenant/listing scope, actor,
  typed event kind, normalized effective locale, note, metadata, timestamp, and
  bounded newest-first timeline reads.
- [x] Register event migration
  `m20260717_000002_create_marketplace_listing_events` with composite tenant/listing
  FK and timeline/kind/actor indexes.
- [x] Persist terms update, submit-for-review, approval/rejection, suspension, and
  archive events atomically with owner state and the durable receipt.
- [x] Include effective locale in the canonical request identity of those evented
  commands.

## Ownership remaining

- [ ] Add immutable `created`, `published`, and `reactivated` events while preserving
  replay admission before seller/product provider reads.
- [ ] Extend event coverage to any future listing matching or moderation commands.
- [ ] Backfill events from mutable `approval_note` and `suspension_reason`
  compatibility snapshots.
- [ ] Drop mutable compatibility snapshot columns only after backfill and full event
  coverage are retained.
- [ ] Publish listing lifecycle/moderation events through the transactional outbox.
- [ ] Add product matching/approval workflow before automated EAN/GTIN matching,
  deduplication, or buy-box ranking.

## FBA completed

- [x] Publish `MarketplaceListingReadPort` for detail, directory, eligibility, and
  bounded event timeline reads.
- [x] Publish `MarketplaceListingCommandPort` for create, terms update, review,
  publication, suspension/reactivation, and archive.
- [x] Resolve seller identity/status through `MarketplaceSellerReadPort`.
- [x] Resolve variant-to-master-product identity through `ProductCatalogReadPort`.
- [x] Require deadlines for reads and deadline plus idempotency key for writes.
- [x] Publish stable safe `PortError` mappings without SQL/driver details.
- [x] Compose root Marketplace listing directory/eligibility consumers without owner
  entities or database access.
- [x] Compile replay-safe command wrappers into the owner crate and route FBA create,
  publish, and reactivate through replay admission before seller/product provider
  reads.
- [x] Route FBA terms update, submit, review, suspend, and archive through evented
  command executors rather than direct compatibility service methods.
- [x] Register `marketplace_listing` in `modules.toml`, distribution, and server as an
  opt-in owner module; Marketplace remains excluded from default module sets.
- [x] Add source guards for schema ownership, absence of localized catalog copy,
  versioned terms, durable receipts, provider-preflight replay, immutable events,
  deterministic eligibility, and module composition.

## FBA remaining

- [ ] Combine provider-preflight replay with atomic `created`, `published`, and
  `reactivated` event persistence.
- [ ] Remove direct compatibility write paths after complete event coverage.
- [ ] Compile owner/provider/root consumer contracts.
- [ ] Apply clean and upgraded SQLite/PostgreSQL migrations.
- [ ] Execute receipt replay, conflicting payload, same-key contention, scope/SKU
  conflicts, terms-version races, lifecycle contention, locale-bound event atomicity,
  bounded timeline isolation, rollback, and restart scenarios.
- [ ] Retain remote-profile timeout/degraded/fallback evidence before promotion.

## FFA remaining

- [ ] Add `rustok-marketplace-listing-admin` with framework-neutral models/core,
  explicit native/GraphQL transport selection, i18n, and a thin Leptos adapter.
- [ ] Add directory/detail, terms history, moderation/lifecycle event history, review,
  publication, suspension, reactivation, archive, and eligibility explanation
  workflows.
- [ ] Add platform `marketplace_listings` permissions without moving owner policy into
  host code.
- [ ] Preserve idempotency keys across retryable command errors.
- [ ] Retain mounted native/GraphQL parity, localized errors, route state, and
  authenticated host evidence before `phase_b_ready`.

## Immediate execution order

1. [ ] Add atomic `created` event to listing creation without moving provider reads
   inside the owner transaction.
2. [ ] Add atomic `published` and `reactivated` events while preserving replay before
   seller reads.
3. [ ] Remove remaining direct FBA compatibility lifecycle paths.
4. [ ] Backfill and drop mutable note compatibility columns.
5. [ ] Publish the listing FFA package and explicit native/GraphQL transports.
6. [ ] Compile and execute database, contention, replay, tenant, locale, restart, and
   mounted transport evidence.

## Source evidence

- `src/entities/listing.rs`
- `src/entities/listing_terms.rs`
- `src/entities/listing_event.rs`
- `src/entities/listing_command_receipt.rs`
- `src/migrations/m20260716_000001_create_marketplace_listings.rs`
- `src/migrations/m20260717_000002_create_marketplace_listing_events.rs`
- `src/command_receipts.rs`
- `src/replay_safe_commands.rs`
- `src/listing_events.rs`
- `src/evented_commands.rs`
- `src/lifecycle_event_commands.rs`
- `src/service.rs`
- `src/ports.rs`
- `contracts/marketplace-listing-fba-registry.json`
- `../rustok-marketplace/src/listing_directory.rs`
- `../rustok-distribution/Cargo.toml`
- `../rustok-distribution/src/lib.rs`
- `../../apps/server/Cargo.toml`
- `../../apps/server/tests/marketplace_listing_boundary_guard.rs`
- `../../apps/server/tests/marketplace_listing_lifecycle_event_guard.rs`
- `../../scripts/verify/verify-marketplace-listing-boundary.mjs`
- `../../scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`

## Promotion gates

- [ ] FBA `boundary_ready`: compiled owner/provider-consumer contracts, complete
  immutable event coverage, durable identity tests, migrations, and source guards
  are retained.
- [ ] FBA `transport_verified`: mounted in-process/remote execution and degraded
  behavior are retained.
- [ ] FFA `phase_b_ready`: module-owned admin package, host composition, and
  native/GraphQL parity evidence are retained.
- [ ] Production-ready: migrations, tenant isolation, contention, restart,
  storefront selection, and operator workflow evidence are retained.
