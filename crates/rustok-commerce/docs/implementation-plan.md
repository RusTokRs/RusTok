# RusToK ecommerce implementation plan

Last reviewed: 2026-07-17

## Source of truth

This file is the only human-maintained source of truth for ecommerce execution
order, completion marks, verification state, and promotion gates.

Rules:

- `[x]` means source or retained execution evidence exists in `main`.
- `[ ]` means implementation or required evidence is still missing.
- Source implementation and runtime verification are separate tasks.
- Local owner plans and FBA registries may contain owner detail but must not
  contradict this plan.
- No FBA or FFA status is promoted from source inspection alone.
- Newly discovered work is recorded here before or with implementation.

`rustok-commerce` owns cross-domain ecommerce orchestration. Product, cart,
customer, region, pricing, inventory, order, payment, fulfillment, tax, promotion,
and market/store remain owner bounded contexts. Marketplace persistence belongs to
the explicit `rustok-marketplace-*` family and must never be folded into
`rustok-commerce`.

## Current boundary

- Ecommerce FFA: `in_progress`.
- Ecommerce FBA: `boundary_ready`.
- Payment FFA: `in_progress`.
- Payment FBA: `boundary_ready`.
- Marketplace family source gate: `open`.
- Marketplace root FFA: `not_started`.
- Marketplace root FBA: `in_progress`.
- Marketplace seller FFA: `in_progress`.
- Marketplace seller FBA: `in_progress`.
- Marketplace listing FFA: `not_started`.
- Marketplace listing FBA: `in_progress`.
- Marketplace production promotion gate: `closed` until compiled contracts,
  clean/upgraded migrations, tenant isolation, contention, restart, mounted
  transports, remote profiles, and financial reconciliation evidence are retained.

## FBA/FFA architecture invariants

- [x] Keep owner persistence, lifecycle policy, receipts, events, and provider policy
  inside owner modules.
- [x] Use typed FBA ports rather than foreign entities or cross-module DB access.
- [x] Carry tenant, actor, effective locale, channel, correlation, deadline, and
  idempotency context across owner calls.
- [x] Keep in-process providers behind the same contracts expected by remote
  adapters.
- [x] Build FFA as module-owned core/model/transport/i18n/thin-UI packages; hosts only
  compose them.
- [x] Require explicit native/GraphQL transport selection; silent fallback is
  forbidden unless explicitly contracted and verified.
- [x] Use `core_only -> core_transport -> core_transport_ui` as the structural
  sequence.
- [x] Keep provider raw payloads, signatures, SQL errors, SDK errors, and KYC raw
  payloads out of owner persistence and public errors.
- [ ] Retain compiled remote-profile evidence before FBA `transport_verified`.
- [ ] Retain mounted native/GraphQL parity before FFA `phase_b_ready`.

## Commerce orchestration

### Checkout

- [x] Require stable checkout idempotency across REST, GraphQL, native, and UI.
- [x] Route production checkout through staged recovery orchestration.
- [x] Resolve cart, product, pricing, inventory, order, payment, and fulfillment
  through owner boundaries.
- [x] Persist immutable plans, operation identity, hashes, lease, stages, errors, and
  owner ids.
- [x] Resume persisted stages and adopt already committed owner outcomes.
- [x] Prevent a second active checkout for the same cart.
- [x] Provide safe compensation and block provider execution during reconciliation.
- [ ] Retain admission parity, kill-point, restart, and PostgreSQL contention evidence.
- [ ] Execute complete mounted operator compensation/reconciliation workflows.

### Return completion

- [x] Keep refund/exchange/claim completion orchestration in commerce.
- [x] Persist durable return-completion operation and immutable command snapshots.
- [x] Atomically admit command and pending operation before provider/owner effects.
- [x] Adopt existing refunds/order changes/completed returns and reject conflicting
  replay payloads.
- [x] Classify uncertain external outcomes as `reconciliation_required`.
- [x] Publish tenant-scoped operator list/show/retry without exposing command payloads.
- [ ] Apply return-completion migrations on clean/upgraded SQLite/PostgreSQL.
- [ ] Execute replay, conflict, admission/claim contention, lease expiry, process-exit,
  restart, and reconciliation-resolution evidence.

## Marketplace Family

### Naming and composition

- [x] Use mandatory `rustok-marketplace-*` crate names and `marketplace_*` slugs.
- [x] Publish `rustok-marketplace` as a composition/orchestration root with no seller,
  listing, allocation, commission, ledger, or payout tables.
- [x] Publish `rustok-marketplace-seller` and `rustok-marketplace-listing` as owner
  modules.
- [x] Keep Marketplace modules opt-in and outside default module/server sets.
- [x] Keep catalog in product, prices in pricing, stock in inventory, customer order
  lifecycle in order, payment/refund state in payment, and generic orchestration in
  commerce.
- [ ] Create future owners as `rustok-marketplace-commission`,
  `rustok-marketplace-ledger`, and `rustok-marketplace-payout`.

### Marketplace root

- [x] Compose seller directory over `MarketplaceSellerReadPort`.
- [x] Compose listing directory and eligibility over `MarketplaceListingReadPort`.
- [x] Keep root consumers free of SeaORM, owner entities, and owner DB access.
- [ ] Compile/execute root consumers and retain remote timeout/degraded/fallback
  evidence.
- [ ] Keep root FFA absent until an aggregate control room can compose owner view
  models without owning policy.

### Seller FBA completed

- [x] Own seller identity, legal profile, onboarding/lifecycle, memberships, roles,
  and seller policy.
- [x] Keep platform RBAC separate from seller membership policy.
- [x] Keep `marketplace_sellers` language-agnostic and localized `display_name` in
  normalized `marketplace_seller_translations`.
- [x] Enforce `(tenant_id, seller_id, locale)`, normalized locale tags, and
  `VARCHAR(32)` storage.
- [x] Use exact effective locale from `PortContext`; owner-side fallback is forbidden.
- [x] Return `resolved_locale` through FBA and FFA projections.
- [x] Create seller, translation, and initial owner membership atomically.
- [x] Persist durable actor-bound command receipts and reject idempotency conflicts.
- [x] Use SQL `ON CONFLICT` for concurrent translation upsert.
- [x] Route all seller FBA writes through the receipt executor; non-receipted write
  bypasses are removed.

### Seller FFA completed

- [x] Publish module-owned seller admin core/model/transport/i18n/Leptos package.
- [x] Implement native and GraphQL source workflows over the same typed ports and
  command envelope.
- [x] Use canonical request effective locale and return `resolved_locale`.
- [x] Preserve idempotency key for explicit retries and forbid implicit transport
  fallback.

### Seller remaining

- [ ] Replace mutable onboarding/suspension prose with immutable locale-tagged seller
  lifecycle/moderation events and bounded timeline reads.
- [ ] Backfill/remove mutable compatibility snapshots after complete event coverage.
- [ ] Publish seller events through the transactional outbox.
- [ ] Add normalized verification facts and KYC provider SPI without raw payloads.
- [ ] Compile seller/core/GraphQL/admin packages, apply migrations, and execute locale,
  replay, tenant, contention, rollback, mounted FFA, and remote-profile evidence.

### Listing FBA completed

- [x] Own seller/master-variant/market/channel identity, seller SKU, versioned terms,
  lifecycle, approval, immutable events, and deterministic eligibility.
- [x] Keep canonical localized product copy, prices, stock, and fulfillment state in
  their owner modules.
- [x] Store no localized product copy or locale-keyed JSON maps in listing tables.
- [x] Use no cross-module DB foreign keys.
- [x] Publish typed listing read/command ports with deadline, error, and idempotency
  rules.
- [x] Resolve seller/product facts through seller and product FBA ports.
- [x] Persist durable listing command receipts and normalized result snapshots.
- [x] Persist append-only `marketplace_listing_events` with actor, effective locale,
  typed event kind, metadata, and bounded newest-first timeline reads.
- [x] Route create, terms update, submit, review, publish, suspend, reactivate, and
  archive through atomic owner state/terms + event + receipt executors.
- [x] Check completed receipt replay before provider reads for create, publish, and
  reactivate, then re-check admission after provider preflight.
- [x] Include effective locale in every listing command identity.
- [x] Keep `MarketplaceListingService` read-only; direct write bypasses are removed.
- [x] Register listing in modules, distribution, and server as opt-in backend owner.

### Listing remaining

- [ ] Define a truthful legacy backfill strategy for `approval_note` and
  `suspension_reason` without fabricating actor or locale facts.
- [ ] Backfill immutable events and drop compatibility columns only after the legacy
  attribution contract is explicit and migration evidence is retained.
- [ ] Publish listing events through transactional outbox ownership.
- [ ] Add product matching/approval before automated EAN/GTIN matching,
  deduplication, or buy-box ranking.
- [ ] Publish `rustok-marketplace-listing-admin` using the FFA
  core/model/transport/i18n/Leptos structure.
- [ ] Compile listing/root/provider contracts and execute migrations, replay,
  provider-preflight races, locale, tenant, event atomicity, contention, rollback,
  restart, and mounted transport evidence.

### Marketplace order allocation and finance

- [ ] Introduce durable seller order groups/allocations without duplicating customer
  order aggregates.
- [ ] Snapshot seller, listing, commission policy/result, fulfillment ownership, and
  monetary allocation at checkout.
- [ ] Prevent one seller lifecycle operation from mutating another seller allocation.
- [ ] Create versioned deterministic commission policy owner.
- [ ] Create immutable double-entry ledger before balances or payouts.
- [ ] Derive all seller balances from ledger entries.
- [ ] Create payout owner with idempotent journals, provider SPI, retries,
  reconciliation, reversals, and operator audit.
- [ ] Keep PSP split-payment optional; internal allocation/ledger correctness must not
  depend on a PSP.

## Payment workstream

- [x] Keep collections, payments, refunds, provider-operation journals, and webhook
  inbox state in payment.
- [x] Publish typed payment collection ports and idempotent refund identity.
- [x] Guard provider operations through the provider registry with CAS journals and
  explicit reconciliation outcomes.
- [x] Route uncertain external outcomes to reconciliation and forbid auto-reclaim.
- [x] Add tenant-scoped Stripe source and deployment-owned secret resolution.
- [x] Mount verified webhook ingress and persist only normalized immutable facts.
- [x] Recover received/failed/expired events; isolate dead letters and require
  operator-only replay.
- [ ] Execute production-like Stripe, real signature, redelivery, restart, replica,
  degraded, reconciliation, and operator evidence.
- [ ] Prove adapters never own payment/refund lifecycle state.

## Verification and promotion checklist

Source inspection is not execution evidence.

### Static

- [ ] `cargo fmt --all -- --check`
- [ ] `npm run verify:ecommerce:fba`
- [ ] `npm run verify:marketplace`
- [ ] `node scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`
- [ ] `cargo xtask module validate marketplace`
- [ ] `cargo xtask module validate marketplace_seller`
- [ ] `cargo xtask module validate marketplace_listing`

### Compile/tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-marketplace --lib`
- [ ] `cargo check -p rustok-marketplace-seller --lib`
- [ ] `cargo check -p rustok-marketplace-seller-admin --all-features`
- [ ] `cargo check -p rustok-marketplace-listing --lib`
- [ ] `cargo check -p rustok-server --features mod-marketplace`
- [ ] Targeted checkout, return-completion, payment, seller/listing lifecycle,
  localization, event timeline, replay, recovery, and tenant-isolation tests.

### Database/runtime

- [ ] Apply clean/upgraded SQLite/PostgreSQL and rollback/reapply paths.
- [ ] Execute receipt/event/provider-operation contention and restart scenarios.
- [ ] Execute seller/listing tenant isolation and cross-locale scenarios.
- [ ] Prove declared routers and module-owned UI packages are mounted.
- [ ] Exercise authenticated checkout, recovery, seller admin, future listing admin,
  reconciliation, and replay.
- [ ] Retain remote-profile and real payment/provider evidence.

## Immediate execution order

1. [x] Complete durable return-completion command admission and operator recovery
   source.
2. [x] Create Marketplace root, seller owner, and seller FFA source.
3. [x] Add seller durable receipts and exact-locale multilingual storage.
4. [x] Create listing owner with versioned terms, receipts, eligibility, and opt-in
   composition.
5. [x] Add complete immutable listing lifecycle-event coverage and remove direct
   service write bypasses.
6. [ ] Define legacy listing note attribution/backfill contract; then backfill and
   drop compatibility columns.
7. [ ] Add immutable seller lifecycle/moderation events and timeline reads.
8. [ ] Create listing FFA package and explicit native/GraphQL transports.
9. [ ] Run static verifiers and fix source drift.
10. [ ] Compile commerce/payment/Marketplace packages and server features.
11. [ ] Apply clean/upgraded migrations and targeted regression tests.
12. [ ] Run contention, restart, kill-point, tenant, locale, and mounted transport
    scenarios.
13. [ ] Introduce seller order allocations, commission snapshots, double-entry ledger,
    and payout journals in that order.
14. [ ] Execute production-like payment provider and mounted worker evidence.
15. [ ] Reassess FBA/FFA promotion strictly from retained evidence.

## Change rules

1. Update this file with every completed or newly discovered ecommerce task.
2. Keep owner plans, registries, manifests, guards, and this plan aligned.
3. Owner modules retain policy, persistence, receipts/events, and commands.
4. Family roots and hosts only compose typed ports and FFA packages.
5. Do not invent legacy actor, locale, provider, or financial facts during migration.
6. Update `docs/modules/registry.md` only when an FFA/FBA status changes.
7. Marketplace names must preserve `rustok-marketplace-*` / `marketplace_*` identity.

## References

- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
- [Marketplace root plan](../../rustok-marketplace/docs/implementation-plan.md)
- [Marketplace seller plan](../../rustok-marketplace-seller/docs/implementation-plan.md)
- [Marketplace listing plan](../../rustok-marketplace-listing/docs/implementation-plan.md)
