# Marketplace family implementation plan

Last reviewed: 2026-07-17

## Status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Root structural shape: `no_ui_boundary`.
- Owner FFA source work: `in_progress`.
- Family source gate: `open`.
- Production promotion gate: `closed`.

Source implementation does not promote a module without retained compile,
migration, contention, mounted-transport, and remote-profile evidence.

## FBA/FFA architecture contract

- [x] Use mandatory `rustok-marketplace-*` crate names and `marketplace_*` slugs.
- [x] Keep the family root as composition/orchestration only; it owns no seller,
  listing, allocation, commission, ledger, or payout tables.
- [x] Keep owner persistence, lifecycle, receipts, events, and provider policy inside
  the corresponding owner module.
- [x] Communicate through typed FBA ports carrying tenant, actor, effective locale,
  channel, correlation, deadline, and idempotency context.
- [x] Keep host applications and FFA packages free of owner policy and owner entities.
- [x] Use `core_only -> core_transport -> core_transport_ui` inside each owner.
- [x] Require explicit native/GraphQL transport selection; silent fallback is
  forbidden.
- [x] Preserve unknown legacy facts as explicit unknown provenance; never fabricate
  actor, locale, provider, or financial attribution during migration.
- [x] Allow hosts to provide request-scoped typed port/runtime composition without
  constructing owner dependencies inside FFA packages.
- [x] Keep owner workflow-to-permission mapping inside owner FFA packages and platform
  allow/deny decisions inside RBAC.
- [ ] Retain compiled in-process/remote-profile evidence before FBA
  `transport_verified`.
- [ ] Retain mounted native/GraphQL parity before FFA `phase_b_ready`.

## Completed family source work

- [x] Publish `rustok-marketplace` as the Marketplace Family root.
- [x] Compose seller directory over `MarketplaceSellerReadPort`.
- [x] Compose listing directory and eligibility over `MarketplaceListingReadPort`.
- [x] Keep root consumers free of SeaORM, foreign entities, and owner DB access.
- [x] Publish `rustok-marketplace-seller` and `rustok-marketplace-listing` owners.
- [x] Register seller, listing, and root modules as opt-in; Marketplace is not
  default-enabled.
- [x] Add family, seller transport, listing lifecycle, provenance, and listing FFA
  source guards.
- [x] Aggregate listing boundary/lifecycle/provenance/FFA source verification into the
  Marketplace and ecommerce verifier chains.

## Owner boundaries

### Seller

- [x] Own language-agnostic seller rows and normalized seller translations.
- [x] Use exact effective locale from `PortContext`; owner fallback is forbidden.
- [x] Persist durable receipts atomically with seller writes and localized snapshots.
- [x] Publish module-owned native/GraphQL FFA source workflows.
- [ ] Replace mutable onboarding/suspension prose with immutable locale-tagged seller
  lifecycle/moderation events and bounded timeline reads.
- [ ] Add normalized verification facts and KYC provider SPI without raw payloads.
- [ ] Retain compiled/mounted FBA and FFA evidence.

### Listing

- [x] Own seller/master-variant/market/channel identity and versioned terms.
- [x] Keep localized product copy, prices, stock, and fulfillment state in owner
  modules.
- [x] Store no localized product copy or locale-keyed JSON maps in listing tables.
- [x] Persist durable receipts and append-only listing events.
- [x] Route all eight FBA writes through atomic state/terms + event + receipt
  executors.
- [x] Preserve replay before seller/product provider reads for create, publish, and
  reactivate.
- [x] Keep the listing service read-only; direct write bypasses are removed.
- [x] Backfill mutable moderation notes as explicit `legacy_snapshot` events with
  nullable actor/locale and source-column metadata.
- [x] Remove `approval_note` and `suspension_reason` from final listing storage and
  DTOs without fabricating legacy attribution.
- [x] Publish `rustok-marketplace-listing-admin` with model/core/transport/i18n/Leptos
  boundaries.
- [x] Preserve command idempotency across explicit UI retry and render legacy history
  as unknown attribution.
- [x] Register the listing FFA crate in the workspace and admin hydrate/SSR feature
  graph while keeping Marketplace backend modules opt-in.
- [x] Add platform `marketplace_listings` permissions and publish the owner-supported
  create/read/list/update/moderate/publish/manage set.
- [x] Require request-scoped typed ports for native FFA composition, return a stable
  error when runtime is unmounted, and fail closed for the unmounted GraphQL profile.
- [ ] Publish listing events through transactional outbox ownership.
- [ ] Mount authenticated native provider composition and real GraphQL transports.
- [ ] Retain compiled/mounted FBA and FFA evidence.

### Future owners

- [ ] Add durable seller order allocations without duplicating the customer order.
- [ ] Add `rustok-marketplace-commission` with versioned deterministic policy
  snapshots.
- [ ] Add immutable double-entry `rustok-marketplace-ledger` before balances/payouts.
- [ ] Add `rustok-marketplace-payout` with idempotent journals, provider SPI,
  reconciliation, reversals, and operator audit.

## FBA promotion

- [ ] Reach family `boundary_ready` after compiled seller/listing contracts,
  clean/upgraded migrations, provenance/event/receipt/outbox contention tests, root
  consumer execution, and retained source guards.
- [ ] Reach family `transport_verified` only after in-process/remote timeout,
  degraded-mode, fallback, and mounted consumer evidence.

## FFA promotion

The root owns no UI. Seller, listing, commission, ledger, and payout UI are published
by owner modules. A future Marketplace control room may only compose owner view
models and transport facades.

- [ ] Seller FFA: retain mounted native/GraphQL parity and localized errors.
- [ ] Listing FFA: mount authenticated native provider composition, add real GraphQL
  roots, and retain native/GraphQL parity.
- [ ] Keep vendor portal, platform admin, and storefront hosts as composition shells.

## Immediate execution order

1. [x] Complete immutable listing events and remove direct write bypasses.
2. [x] Backfill truthful legacy snapshots and remove mutable note columns.
3. [x] Publish listing FFA source, workspace/admin wiring, and listing RBAC.
4. [ ] Define and atomically publish the versioned listing outbox event.
5. [ ] Complete immutable seller lifecycle/moderation events.
6. [ ] Mount authenticated listing native provider composition and real GraphQL roots.
7. [ ] Synchronize the central module readiness board safely.
8. [ ] Compile seller/listing/root contracts and apply SQLite/PostgreSQL migrations.
9. [ ] Execute idempotency, provenance, locale, tenant, outbox, contention, restart,
   and mounted transport scenarios.
10. [ ] Start seller order allocation, commission, ledger, and payout owners in order.

## Source evidence

- `src/seller_directory.rs`
- `src/listing_directory.rs`
- `contracts/marketplace-fba-registry.json`
- `../rustok-marketplace-seller/docs/implementation-plan.md`
- `../rustok-marketplace-listing/docs/implementation-plan.md`
- `../rustok-marketplace-listing/src/migrations/m20260717_000003_backfill_listing_event_provenance.rs`
- `../rustok-marketplace-listing/admin/src/model.rs`
- `../rustok-marketplace-listing/admin/src/transport.rs`
- `../rustok-marketplace-listing/admin/src/transport/native_server_adapter.rs`
- `../rustok-marketplace-listing/admin/src/transport/graphql_adapter.rs`
- `../rustok-marketplace-listing/admin/src/ui/leptos.rs`
- `../../Cargo.toml`
- `../../apps/admin/Cargo.toml`
- `../../apps/server/tests/marketplace_family_boundary_guard.rs`
- `../../apps/server/tests/marketplace_seller_transport_guard.rs`
- `../../apps/server/tests/marketplace_listing_boundary_guard.rs`
- `../../apps/server/tests/marketplace_listing_lifecycle_event_guard.rs`
- `../../apps/server/tests/marketplace_listing_provenance_cutover_guard.rs`
- `../../apps/server/tests/marketplace_listing_admin_ffa_guard.rs`
- `../../scripts/verify/verify-marketplace-family-boundary.mjs`
- `../../scripts/verify/verify-marketplace-seller-transport.mjs`
- `../../scripts/verify/verify-marketplace-listing-boundary.mjs`
- `../../scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`
- `../../scripts/verify/verify-marketplace-listing-provenance-cutover.mjs`
- `../../scripts/verify/verify-marketplace-listing-admin-ffa.mjs`
