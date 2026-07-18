# Marketplace seller implementation plan

Last reviewed: 2026-07-18

## Status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Owner source gate: `open`.
- Production promotion gate: `closed`.

Source implementation does not promote FBA or FFA without retained compile,
migration, contention, mounted-transport, and remote-profile evidence.

## FBA/FFA architecture contract

- [x] Keep seller persistence, membership policy, lifecycle, receipts, translations,
  immutable events, and future verification facts inside `rustok-marketplace-seller`.
- [x] Expose owner behavior through typed FBA read/command ports using
  `PortContext` and stable `PortError` mappings.
- [x] Keep native and GraphQL FFA adapters over the same command envelope and owner
  ports; host applications do not implement seller policy.
- [x] Use explicit transport selection and forbid implicit native/GraphQL fallback.
- [x] Treat effective locale as request context, not as a host or UI default.
- [ ] Retain compiled remote-profile contract evidence before FBA
  `transport_verified`.
- [ ] Retain mounted native/GraphQL parity before FFA `phase_b_ready`.

## Ownership completed

- [x] Own seller identity, legal profile, lifecycle, and onboarding state.
- [x] Own seller-scoped memberships, roles, and status.
- [x] Keep platform RBAC access separate from seller membership policy.
- [x] Create seller, effective-locale translation, and initial owner membership in one
  database transaction.
- [x] Keep `marketplace_sellers` language-agnostic; store localized `display_name`
  only in `marketplace_seller_translations`.
- [x] Enforce `(tenant_id, seller_id, locale)` translation identity, normalized locale
  tags, and `VARCHAR(32)` locale storage.
- [x] Use exact effective locale supplied by `PortContext`; the owner does not invent
  a fallback chain after request middleware resolution.
- [x] Return `resolved_locale` with seller projections.
- [x] Enforce tenant-scoped handle and membership identities in the schema.
- [x] Use expected-state updates for onboarding and suspension transitions.
- [x] Persist one tenant-scoped command receipt for every FBA seller write with
  immutable command kind, actor-bound canonical SHA-256 request identity, normalized
  typed result snapshot, and completed timestamp.
- [x] Include effective locale in localized command request identity and commit the
  translation upsert, owner mutation, receipt, and response snapshot atomically.
- [x] Use database `ON CONFLICT` for translation upsert so concurrent locale writes do
  not abort a PostgreSQL transaction after a unique violation.
- [x] Reject reuse of an idempotency key for another command kind, actor, locale, or
  payload.
- [x] Own append-only `marketplace_seller_events` with tenant/seller scope, typed event
  kind, nullable attribution only for explicit legacy snapshots, note, metadata, and
  timestamp.
- [x] Enforce truthful command/legacy attribution with a database CHECK and composite
  tenant/seller foreign key.
- [x] Publish a bounded newest-first owner timeline read through
  `MarketplaceSellerReadPort::list_seller_events`.
- [x] Commit onboarding review, suspension, and reactivation state, immutable command
  event, completed receipt, and normalized response snapshot in one transaction.
- [x] Keep lost-response replay outside event append so one idempotency key produces
  exactly one lifecycle event.
- [x] Roll back owner state and the pending receipt when lifecycle event persistence
  fails.

## Ownership remaining

- [ ] Route create, profile update, onboarding submit, and member writes through atomic
  state + event + receipt transactions.
- [ ] Backfill existing onboarding/suspension prose snapshots and remove mutable
  compatibility columns only after live command event coverage is complete.
- [ ] Publish seller lifecycle events through the transactional outbox.
- [ ] Add normalized verification facts and a KYC provider SPI without raw provider
  payload persistence.

## FBA completed

- [x] Publish `MarketplaceSellerReadPort` with deadline semantics for seller,
  directory, membership, member-list, and bounded event timeline reads.
- [x] Publish `MarketplaceSellerCommandPort` with deadline and idempotency-key
  admission semantics.
- [x] Route every command-port implementation through the durable receipt executor;
  direct owner write methods are not an FBA path.
- [x] Route localized reads and writes through exact `PortContext.locale` resolution.
- [x] Map a missing effective-locale translation to the stable
  `marketplace_seller.translation_missing` invariant error.
- [x] Publish stable typed error mapping without SQL/driver details.
- [x] Publish the in-process provider registry and planned remote-adapter cases.
- [x] Add source guards for multilingual schema, exact locale resolution, replay,
  conflict, typed response snapshots, transport wiring, immutable event storage,
  truthful provenance, bounded timeline reads, lifecycle completion order, and
  root/owner non-bypass rules.
- [x] Execute SQLite proof for one-event replay semantics and event-insert rollback of
  lifecycle state plus the pending receipt.
- [x] Aggregate marketplace family and seller transport verifiers into the root npm
  verification entry points.

## FBA remaining

- [ ] Extend atomic immutable event production to create/profile/onboarding-submit and
  member commands.
- [ ] Compile the provider and GraphQL contracts.
- [ ] Apply seller/translation/receipt/event migrations on clean and upgraded
  SQLite/PostgreSQL graphs.
- [ ] Execute same-key contention, lifecycle contention, cross-tenant, and PostgreSQL
  event atomicity scenarios.
- [ ] Retain timeout, degraded, remote-profile, and fallback execution evidence before
  promoting FBA to `transport_verified`.

## FFA completed

- [x] Create module-owned `rustok-marketplace-seller-admin` package boundaries for
  core, model, transport, i18n, and Leptos UI.
- [x] Implement authenticated native server transport over typed seller ports with
  tenant and marketplace-seller permission checks.
- [x] Implement module-owned GraphQL query/mutation roots and a selected GraphQL admin
  adapter over the same command envelope.
- [x] Use canonical request `RequestContext.locale` in native and GraphQL transports
  instead of reconstructing locale from tenant defaults.
- [x] Return `resolved_locale` through native and GraphQL admin DTOs.
- [x] Keep native/GraphQL selection explicit through `execute_selected_transport`;
  automatic fallback is forbidden.
- [x] Complete source workflows for seller directory/detail, create/profile,
  onboarding submit/review, suspend/reactivate, member invite, and member status.
- [x] Preserve the original idempotency key after a command error and expose an
  explicit retry action that reuses the same command and key.
- [x] Wire seller GraphQL roots into module manifest/server features and seller UI
  into manifest-driven admin host composition without default-enabling Marketplace.

## FFA remaining

- [ ] Add lifecycle/moderation history to native and GraphQL DTOs and workflows over
  the new owner event read operation.
- [ ] Retain native/GraphQL parity, localized errors, route state, retries, and mounted
  authenticated host evidence before promoting FFA to `phase_b_ready`.

## Immediate execution order

1. [x] Add immutable seller lifecycle/moderation event schema, entity, typed DTOs, and
   bounded FBA timeline read.
2. [x] Route onboarding review, suspension, and reactivation through atomic
   state + event + receipt transactions.
3. [ ] Extend event production to create/profile/onboarding-submit/member commands.
4. [ ] Add event history to native and GraphQL FFA transports.
5. [ ] Backfill/remove mutable prose snapshots.
6. [ ] Publish live seller events through the transactional outbox.
7. [ ] Add normalized verification/KYC facts and provider SPI.
8. [ ] Compile and execute database, contention, replay, tenant, and mounted transport
   evidence.

## Source evidence

- `src/entities/seller.rs`
- `src/entities/seller_translation.rs`
- `src/entities/seller_command_receipt.rs`
- `src/entities/seller_event.rs`
- `src/migrations/m20260716_000001_create_marketplace_sellers.rs`
- `src/migrations/m20260716_000002_create_seller_command_receipts.rs`
- `src/migrations/m20260718_000003_create_marketplace_seller_events.rs`
- `src/localized_sellers.rs`
- `src/command_receipts.rs`
- `src/receipted_commands.rs`
- `src/seller_events.rs`
- `src/seller_events_tests.rs`
- `src/ports.rs`
- `src/graphql.rs`
- `admin/src/model.rs`
- `admin/src/transport.rs`
- `admin/src/transport/native_server_adapter.rs`
- `admin/src/transport/graphql_adapter.rs`
- `admin/src/ui/leptos.rs`
- `contracts/marketplace-seller-fba-registry.json`
- `../../apps/server/tests/marketplace_family_boundary_guard.rs`
- `../../apps/server/tests/marketplace_seller_transport_guard.rs`
- `../../scripts/verify/verify-marketplace-family-boundary.mjs`
- `../../scripts/verify/verify-marketplace-seller-events.mjs`
- `../../scripts/verify/verify-marketplace-seller-transport.mjs`
