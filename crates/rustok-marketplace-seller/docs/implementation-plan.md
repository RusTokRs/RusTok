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
- [ ] Add immutable seller lifecycle/audit events through the transactional outbox.
- [ ] Add normalized verification facts and a KYC provider SPI without raw payload
  persistence.
- [ ] Add durable command receipts so FBA idempotency keys protect replay after a
  successful owner write and lost response.

## FBA

- [x] Publish `MarketplaceSellerReadPort` with deadline semantics.
- [x] Publish `MarketplaceSellerCommandPort` with deadline and idempotency-key
  admission semantics.
- [x] Publish stable typed error mapping without SQL/driver details.
- [x] Publish the in-process provider registry and planned remote-adapter cases.
- [ ] Add durable command replay and conflicting-payload detection.
- [ ] Add source guard aggregation and compile the provider contract.
- [ ] Retain timeout, degraded, remote-profile, and fallback execution evidence
  before promoting FBA to `transport_verified`.

## FFA

- [x] Create module-owned `rustok-marketplace-seller-admin` package boundaries for
  core, model, transport, i18n, and Leptos UI.
- [ ] Implement authenticated native server transport over the owner service.
- [ ] Implement GraphQL-compatible selected transport without automatic fallback.
- [ ] Complete seller list/detail/onboarding/suspension/member workflows.
- [ ] Retain native/GraphQL parity, localized errors, route state, and mounted host
  evidence before promoting FFA to `phase_b_ready`.

## Database and runtime evidence

- [ ] Apply clean and upgraded SQLite/PostgreSQL migrations.
- [ ] Verify rollback/reapply and composite tenant/seller foreign keys.
- [ ] Execute duplicate handle/member, concurrent review/suspend, and cross-tenant
  access scenarios.
- [ ] Prove seller writes cannot mutate another tenant or seller scope.
