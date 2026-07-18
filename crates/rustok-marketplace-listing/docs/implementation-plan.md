# Marketplace listing implementation plan

Last reviewed: 2026-07-17

## Status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Owner source gate: `open`.
- Production promotion gate: `closed`.

Source implementation does not promote FBA or FFA without retained compile,
migration, contention, mounted-transport, and remote-profile evidence.

## FBA/FFA architecture contract

- [x] Keep listing identity, versioned terms, lifecycle, receipts, events, and
  eligibility inside `rustok-marketplace-listing`.
- [x] Resolve seller and product facts through typed FBA ports; do not import foreign
  entities or add cross-module database foreign keys.
- [x] Keep product-owned localized title/description/translations outside listing
  storage.
- [x] Carry effective locale through `PortContext` for every command-origin event.
- [x] Keep the Marketplace root and FFA hosts as composition surfaces only.
- [x] Keep `MarketplaceListingService` read-only; owner writes live only in
  receipt/event executors.
- [x] Preserve unknown legacy attribution as explicit nullable facts with typed
  provenance instead of fabricating actor or locale.
- [x] Publish a module-owned listing FFA package with framework-neutral
  model/core/transport/i18n boundaries and a thin Leptos adapter.
- [x] Require explicit native/GraphQL transport selection; implicit fallback is
  forbidden.
- [x] Require native FFA hosts to provide request-scoped typed listing ports,
  authorization, and canonical `PortContext` construction.
- [x] Keep owner workflow-to-permission mapping inside the listing FFA while platform
  RBAC remains responsible for the allow/deny decision.
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
- [x] Add append-only `marketplace_listing_events` with tenant/listing scope, typed
  event kind, provenance, note, metadata, timestamp, and bounded newest-first reads.
- [x] Require actor and normalized effective locale for `command` events.
- [x] Allow nullable actor/locale only for explicit `legacy_snapshot` provenance and
  reject fabricated attribution in the read mapper and database constraint.
- [x] Persist `created`, `terms_updated`, `submitted_for_review`, `approved`,
  `rejected`, `published`, `suspended`, `reactivated`, and `archived` events
  atomically with owner state/terms and the durable command receipt.
- [x] Publish one sealed external listing contract event from the receipt completion
  executor in the same transaction as owner state, internal event, and completed
  receipt.
- [x] Keep completed receipt replay outside the publication path and roll back the
  owner transaction when contract mapping or outbox persistence fails.
- [x] Preserve replay admission before seller/product provider reads for create,
  publish, and reactivate.
- [x] Remove all direct write methods from the read/provider composition service.
- [x] Add irreversible migration
  `m20260717_000003_backfill_listing_event_provenance`.
- [x] Import non-empty legacy `approval_note` and `suspension_reason` values as typed
  legacy snapshot events with source-column metadata and no fabricated actor/locale.
- [x] Remove mutable `approval_note` and `suspension_reason` from the final entity,
  response DTO, write paths, and post-cutover database schema.

## Ownership remaining

- [x] Define the sealed, typed nine-variant `MarketplaceListingEvent` family in
  `rustok-events` before external Marketplace consumers subscribe.
- [x] Keep moderation notes and arbitrary owner metadata out of the external event;
  consumers refresh through `MarketplaceListingReadPort`.
- [x] Do not relay imported legacy snapshots as new live business commands.
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
- [x] Include effective locale in every command request identity.
- [x] Check completed receipt replay before provider reads for create, publish, and
  reactivate; re-check admission after provider preflight to handle races.
- [x] Register `marketplace_listing` in `modules.toml`, distribution, and server as an
  opt-in owner module; Marketplace remains excluded from default module sets.
- [x] Add source guards for schema ownership, absence of localized catalog copy,
  versioned terms, durable receipts, provider-preflight replay, complete immutable
  events, truthful provenance cutover, sealed external event contracts, owner outbox
  publication, read-service non-bypass, deterministic eligibility, and composition.
- [x] Restore evented module registration and command-port routing after parallel
  source drift; compatibility write methods are not an FBA path.
- [x] Add the sealed versioned listing event family to `rustok-events`, including
  validation, schema registry coverage, serialization safety tests, source guards,
  and the generic transactional outbox publication boundary.
- [x] Publish the event family through
  `TransactionalEventBus::publish_contract_in_tx` before receipt completion and
  transaction commit.

## FBA remaining

- [ ] Compile owner/provider/root consumer contracts.
- [ ] Apply clean and upgraded SQLite/PostgreSQL migrations, including the
  intentionally irreversible provenance cutover.
- [ ] Execute receipt replay, conflicting payload, same-key contention, provider
  preflight races, scope/SKU conflicts, terms-version races, lifecycle contention,
  locale/provenance constraints, bounded timeline isolation, PostgreSQL outbox
  atomicity, rollback, relay, and restart scenarios.
- [ ] Retain remote-profile timeout/degraded/fallback evidence before promotion.

## FFA completed

- [x] Add `rustok-marketplace-listing-admin` with framework-neutral models/core,
  explicit native/GraphQL transport selection, English/Russian catalogs, and a thin
  Leptos adapter.
- [x] Model directory/detail, current terms, immutable lifecycle/moderation history,
  create, terms update, submit, review, publish, suspend, reactivate, and archive.
- [x] Show legacy snapshots as unknown-attribution history rather than pretending they
  are command events.
- [x] Preserve the original command and idempotency key for explicit retry after a
  transport error.
- [x] Keep native UI composition behind a request-scoped host runtime containing typed
  listing ports, an authorizer, and canonical `PortContext` construction.
- [x] Fail closed with a stable server-function error when the native runtime is not
  mounted instead of panicking or constructing owner dependencies in the UI package.
- [x] Declare the GraphQL profile as explicitly unmounted instead of silently falling
  back to native or inventing unsupported schema operations.
- [x] Register the module-owned admin package and locale path in
  `rustok-module.toml`.
- [x] Register the nested admin crate in the workspace and the admin hydrate/SSR
  feature graph.
- [x] Add platform `marketplace_listings` permissions and publish the supported
  permission set from the owner module.
- [x] Map listing FFA actions to create/read/list/update/moderate/publish/manage
  permissions; delete is not exposed and archive remains an owner command.
- [x] Add and aggregate `marketplace_listing_admin_ffa_guard.rs` plus
  `verify-marketplace-listing-admin-ffa.mjs`.

## FFA remaining

- [ ] Provide authenticated request-scoped listing provider/runtime composition from
  admin hosts.
- [ ] Publish module-owned listing GraphQL query/mutation roots over the same typed
  ports, then replace the declared-unmounted GraphQL adapter with real operations.
- [ ] Add eligibility explanation and paginated event/terms history refinements.
- [ ] Synchronize `docs/modules/registry.md` after a safe full-document update.
- [ ] Retain mounted native/GraphQL parity, localized errors, route state, and
  authenticated host evidence before `phase_b_ready`.

## Immediate execution order

1. [x] Complete immutable events for all FBA write commands.
2. [x] Backfill truthful legacy snapshots and remove mutable note columns.
3. [x] Publish the initial module-owned listing FFA source package.
4. [x] Add workspace/admin feature wiring and platform listing permissions.
5. [x] Atomically publish the versioned event family through the transactional outbox.
6. [ ] Mount authenticated request-scoped native provider composition.
7. [ ] Add listing GraphQL roots and replace the declared-unmounted adapter.
8. [ ] Compile and execute database, contention, replay, tenant, locale, provenance,
   outbox, restart, and mounted transport evidence.
9. [ ] Start product matching/approval only after owner/runtime evidence is retained.

## Source evidence

- `src/entities/listing.rs`
- `src/entities/listing_terms.rs`
- `src/entities/listing_event.rs`
- `src/entities/listing_command_receipt.rs`
- `src/migrations/m20260716_000001_create_marketplace_listings.rs`
- `src/migrations/m20260717_000002_create_marketplace_listing_events.rs`
- `src/migrations/m20260717_000003_backfill_listing_event_provenance.rs`
- `src/command_receipts.rs`
- `src/command_receipts_tests.rs`
- `src/external_events.rs`
- `src/replay_safe_commands.rs`
- `src/listing_events.rs`
- `src/evented_commands.rs`
- `src/lifecycle_event_commands.rs`
- `src/service.rs`
- `src/ports.rs`
- `admin/Cargo.toml`
- `admin/src/model.rs`
- `admin/src/core.rs`
- `admin/src/transport.rs`
- `admin/src/transport/native_server_adapter.rs`
- `admin/src/transport/graphql_adapter.rs`
- `admin/src/ui/leptos.rs`
- `admin/locales/en.json`
- `admin/locales/ru.json`
- `contracts/marketplace-listing-fba-registry.json`
- `../rustok-marketplace/src/listing_directory.rs`
- `../../Cargo.toml`
- `../../apps/admin/Cargo.toml`
- `../../apps/server/tests/marketplace_listing_boundary_guard.rs`
- `../../apps/server/tests/marketplace_listing_lifecycle_event_guard.rs`
- `../../apps/server/tests/marketplace_listing_event_contract_guard.rs`
- `../../apps/server/tests/marketplace_listing_provenance_cutover_guard.rs`
- `../../apps/server/tests/marketplace_listing_admin_ffa_guard.rs`
- `../../scripts/verify/verify-marketplace-listing-boundary.mjs`
- `../../scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`
- `../../scripts/verify/verify-marketplace-listing-event-contract.mjs`
- `../../rustok-events/src/contract.rs`
- `../../rustok-events/src/marketplace_listing.rs`
- `../../rustok-events/tests/marketplace_listing_contracts.rs`
- `../../rustok-outbox/src/transactional.rs`
- `../../rustok-outbox/src/transport.rs`
- `../../DECISIONS/2026-07-17-sealed-typed-event-families.md`
- `../../scripts/verify/verify-marketplace-listing-provenance-cutover.mjs`
- `../../scripts/verify/verify-marketplace-listing-admin-ffa.mjs`

## Promotion gates

- [ ] FBA `boundary_ready`: compiled owner/provider-consumer contracts, durable
  identity/provenance/outbox tests, migrations, and source guards are retained.
- [ ] FBA `transport_verified`: mounted in-process/remote execution and degraded
  behavior are retained.
- [ ] FFA `phase_b_ready`: authenticated native runtime composition, real GraphQL
  transport, and parity evidence are retained.
- [ ] Production-ready: migrations, tenant isolation, contention, restart,
  storefront selection, and operator workflow evidence are retained.
