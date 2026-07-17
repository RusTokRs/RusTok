# Marketplace seller implementation plan

Last reviewed: 2026-07-17

## Status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Owner source gate: `open`.
- Production promotion gate: `closed`.

## Ownership

- [x] Own seller identity, legal profile, lifecycle, and onboarding state.
- [x] Own seller-scoped memberships, roles, and status.
- [x] Keep platform RBAC access separate from seller membership policy.
- [x] Create seller, effective-locale translation, and initial owner membership in one
  database transaction.
- [x] Keep `marketplace_sellers` language-agnostic; store localized `display_name`
  only in `marketplace_seller_translations`.
- [x] Enforce `(tenant_id, seller_id, locale)` translation identity, normalized locale
  tags, and `VARCHAR(32)` locale storage.
- [x] Use the effective locale supplied by `PortContext`; the owner does not invent a
  fallback chain after request middleware resolution.
- [x] Return `resolved_locale` with seller projections so native and GraphQL clients
  can prove which localized row was used.
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
- [ ] Replace mutable onboarding/suspension prose with immutable seller lifecycle and
  moderation events carrying actor and effective locale; these operator records are
  not seller profile translations.
- [ ] Publish lifecycle events through the transactional outbox.
- [ ] Add normalized verification facts and a KYC provider SPI without raw payload
  persistence.

## FBA

- [x] Publish `MarketplaceSellerReadPort` with deadline semantics for seller,
  directory, membership, and member-list reads.
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
  conflict, typed response snapshots, transport wiring, and root/owner non-bypass
  rules.
- [x] Aggregate marketplace family and seller transport verifiers into the root npm
  verification entry points.
- [ ] Compile the provider and GraphQL contracts.
- [ ] Apply seller/translation/receipt migrations and execute lost-response replay,
  conflicting payload, same-key contention, same-locale upsert contention, and
  rollback scenarios on SQLite/PostgreSQL.
- [ ] Retain timeout, degraded, remote-profile, and fallback execution evidence
  before promoting FBA to `transport_verified`.

## FFA

- [x] Create module-owned `rustok-marketplace-seller-admin` package boundaries for
  core, model, transport, i18n, and Leptos UI.
- [x] Implement authenticated native server transport over typed seller ports with
  tenant and marketplace-seller permission checks.
- [x] Implement module-owned GraphQL query/mutation roots and a selected GraphQL
  admin adapter over the same command envelope.
- [x] Use canonical request `RequestContext.locale` in native and GraphQL owner
  transports instead of reconstructing locale from tenant defaults.
- [x] Return `resolved_locale` through native and GraphQL admin DTOs.
- [x] Keep native/GraphQL selection explicit through `execute_selected_transport`;
  automatic fallback is forbidden.
- [x] Complete source workflows for seller directory/detail, create/profile,
  onboarding submit/review, suspend/reactivate, member invite, and member status.
- [x] Preserve the original idempotency key after a command error and expose an
  explicit retry action that reuses the same command and key.
- [x] Wire seller GraphQL roots into module manifest/server features and seller UI
  into manifest-driven admin host composition without default-enabling Marketplace.
- [ ] Retain native/GraphQL parity, localized errors, route state, and mounted host
  evidence before promoting FFA to `phase_b_ready`.

## Source evidence

- `src/entities/seller.rs`
- `src/entities/seller_translation.rs`
- `src/entities/seller_command_receipt.rs`
- `src/migrations/m20260716_000001_create_marketplace_sellers.rs`
- `src/migrations/m20260716_000002_create_seller_command_receipts.rs`
- `src/localized_sellers.rs`
- `src/command_receipts.rs`
- `src/receipted_commands.rs`
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
- `../../scripts/verify/verify-marketplace-seller-transport.mjs`

## Database and runtime evidence

- [ ] Apply clean and upgraded SQLite/PostgreSQL migrations.
- [ ] Verify rollback/reapply, translation uniqueness, and composite tenant/seller
  foreign keys.
- [ ] Execute duplicate handle/member, concurrent review/suspend, receipt replay,
  exact-locale missing translation, parallel-locale update, same-locale upsert, and
  cross-tenant access scenarios.
- [ ] Prove seller writes cannot mutate another tenant, seller, or locale scope.
- [ ] Execute mounted native and GraphQL workflows with authenticated operators.
