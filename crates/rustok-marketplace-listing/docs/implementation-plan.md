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
- [x] Carry effective locale through `PortContext` for every event-producing command.
- [x] Keep the Marketplace root and future FFA hosts as composition surfaces only.
- [x] Keep `MarketplaceListingService` read-only; owner writes live only in
  receipt/event executors.
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
- [x] Persist `created`, `terms_updated`, `submitted_for_review`, `approved`,
  `rejected`, `published`, `suspended`, `reactivated`, and `archived` events
  atomically with owner state/terms and the durable command receipt.
- [x] Include effective locale in the canonical request identity of every listing
  command.
- [x] Preserve replay admission before seller/product provider reads for create,
  publish, and reactivate.
- [x] Remove all direct write methods from the read/provider composition service.

## Ownership remaining

- [ ] Backfill events from mutable `approval_note` and `suspension_reason`
  compatibility snapshots.
- [ ] Drop mutable compatibility snapshot columns after backfill and retained
  migration evidence.
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
- [x] Route all eight FBA write operations through durable receipt/event executors.
- [x] Check completed receipt replay before provider reads for create, publish, and
  reactivate; re-check admission after provider preflight to handle races.
- [x] Register `marketplace_listing` in `modules.toml`, distribution, and server as an
  opt-in owner module; Marketplace remains excluded from default module sets.
- [x] Add source guards for schema ownership, absence of localized catalog copy,
  versioned terms, durable receipts, provider-preflight replay, complete immutable
  events, read-service non-bypass, deterministic eligibility, and module composition.

## FBA remaining

- [ ] Backfill/drop compatibility snapshot columns without changing normalized
  response semantics unexpectedly.
- [ ] Compile owner/provider/root consumer contracts.
- [ ] Apply clean and upgraded SQLite/PostgreSQL migrations.
- [ ] Execute receipt replay, conflicting payload, same-key contention, provider
  preflight races, scope/SKU conflicts, terms-version races, lifecycle contention,
  locale-bound event atomicity, bounded timeline isolation, rollback, and restart
  scenarios.
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

1. [ ] Add compatibility backfill/drop migration for `approval_note` and
   `suspension_reason`.
2. [ ] Publish listing lifecycle events through transactional outbox ownership.
3. [ ] Publish the listing FFA package and explicit native/GraphQL transports.
4. [ ] Add platform listing permissions and module-owned admin workflows.
5. [ ] Compile and execute database, contention, replay, tenant, locale, restart, and
   mounted transport evidence.
6. [ ] Start product matching/approval only after owner/runtime evidence is retained.

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

- [ ] FBA `boundary_ready`: compiled owner/provider-consumer contracts, durable
  identity tests, compatibility-column cutover, migrations, and source guards are
  retained.
- [ ] FBA `transport_verified`: mounted in-process/remote execution and degraded
  behavior are retained.
- [ ] FFA `phase_b_ready`: module-owned admin package, host composition, and
  native/GraphQL parity evidence are retained.
- [ ] Production-ready: migrations, tenant isolation, contention, restart,
  storefront selection, and operator workflow evidence are retained.
