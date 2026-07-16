# Marketplace seller implementation plan

Last reviewed: 2026-07-16

## Status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Owner source gate: `open`.
- Production promotion gate: `closed`.

## Ownership

- [x] Own seller identity, display/legal profile, lifecycle, and onboarding state.
- [x] Own seller-scoped memberships, roles, and status.
- [x] Keep platform RBAC access separate from seller membership policy.
- [x] Create seller and initial owner membership in one database transaction.
- [x] Enforce tenant-scoped handle and membership identities in the schema.
- [x] Use expected-state updates for onboarding and suspension transitions.
- [x] Persist one tenant-scoped command receipt for every FBA seller write with
  immutable command kind, actor-bound canonical SHA-256 request identity, normalized
  typed result snapshot, and completed timestamp.
- [x] Commit the command receipt, owner mutation, and response snapshot in the same
  database transaction so a lost response replays the saved owner result.
- [x] Reject reuse of an idempotency key for another command kind, actor, or payload.
- [ ] Add immutable seller lifecycle/audit events through the transactional outbox.
- [ ] Add normalized verification facts and a KYC provider SPI without raw payload
  persistence.

## FBA

- [x] Publish `MarketplaceSellerReadPort` with deadline semantics for seller,
  directory, membership, and member-list reads.
- [x] Publish `MarketplaceSellerCommandPort` with deadline and idempotency-key
  admission semantics.
- [x] Route every command-port implementation through the durable receipt executor;
  direct owner write methods are not an FBA path.
- [x] Publish stable typed error mapping without SQL/driver details.
- [x] Publish the in-process provider registry and planned remote-adapter cases.
- [x] Add source guards for schema, replay, conflict, typed response snapshots,
  transport wiring, and root/owner non-bypass rules.
- [x] Aggregate marketplace family and seller transport verifiers into the root npm
  verification entry points.
- [ ] Compile the provider and GraphQL contracts.
- [ ] Apply the receipt migration and execute lost-response replay, conflicting
  payload, same-key contention, and rollback scenarios on SQLite/PostgreSQL.
- [ ] Retain timeout, degraded, remote-profile, and fallback execution evidence
  before promoting FBA to `transport_verified`.

## FFA

- [x] Create module-owned `rustok-marketplace-seller-admin` package boundaries for
  core, model, transport, i18n, and Leptos UI.
- [x] Implement authenticated native server transport over typed seller ports with
  tenant and marketplace-seller permission checks.
- [x] Implement module-owned GraphQL query/mutation roots and a selected GraphQL
  admin adapter over the same command envelope.
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

- `src/entities/seller_command_receipt.rs`
- `src/migrations/m20260716_000002_create_seller_command_receipts.rs`
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
- [ ] Verify rollback/reapply and composite tenant/seller foreign keys.
- [ ] Execute duplicate handle/member, concurrent review/suspend, receipt replay,
  and cross-tenant access scenarios.
- [ ] Prove seller writes cannot mutate another tenant or seller scope.
- [ ] Execute mounted native and GraphQL workflows with authenticated operators.
