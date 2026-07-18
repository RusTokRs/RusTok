# RusToK ecommerce implementation plan

Last reviewed: 2026-07-18

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
- Legacy migrations must not invent actor, locale, provider, or financial facts.

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
- Marketplace family FFA: `in_progress`.
- Marketplace root FFA: `not_started`.
- Marketplace root FBA: `in_progress`.
- Marketplace seller FFA: `in_progress`.
- Marketplace seller FBA: `in_progress`.
- Marketplace listing FFA: `in_progress`.
- Marketplace listing FBA: `in_progress`.
- Marketplace production gate: `closed` until compiled contracts, clean/upgraded
  migrations, tenant isolation, contention, restart, mounted transports, remote
  profiles, and financial reconciliation evidence are retained.

## FBA/FFA architecture invariants

- [x] Keep owner persistence, lifecycle policy, receipts, events, and provider policy
  inside owner modules.
- [x] Use typed FBA ports rather than foreign entities or cross-module DB access.
- [x] Carry tenant, actor, effective locale, channel, correlation, deadline, and
  idempotency context across owner calls.
- [x] Keep in-process providers behind the same contracts expected by remote adapters.
- [x] Build FFA as module-owned core/model/transport/i18n/thin-UI packages; hosts only
  compose them.
- [x] Require explicit native/GraphQL transport selection; silent fallback is
  forbidden unless explicitly contracted and verified.
- [x] Use `core_only -> core_transport -> core_transport_ui`.
- [x] Keep provider raw payloads, signatures, SQL errors, SDK errors, and KYC raw
  payloads out of owner persistence and public errors.
- [x] Preserve unknown historical attribution as typed unknown provenance rather than
  sentinel UUIDs or guessed locales.
- [x] Allow request-scoped hosts to inject typed owner ports, authorization, and
  canonical `PortContext`; FFA packages must not construct foreign providers.
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
- [x] Publish `rustok-marketplace` as a composition/orchestration root with no owner
  tables.
- [x] Publish `rustok-marketplace-seller` and `rustok-marketplace-listing` as owner
  modules.
- [x] Keep Marketplace modules opt-in and outside default module/server sets.
- [x] Keep catalog, prices, stock, orders, payments, and generic orchestration in their
  existing owner modules.
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
- [x] Keep base seller rows language-agnostic and localized `display_name` in
  normalized translation rows.
- [x] Enforce tenant/seller/locale identity, normalized locale tags, and
  `VARCHAR(32)` storage.
- [x] Use exact effective locale from `PortContext`; owner-side fallback is forbidden.
- [x] Return `resolved_locale` through FBA and FFA projections.
- [x] Create seller, translation, and initial owner membership atomically.
- [x] Persist durable actor-bound command receipts and reject idempotency conflicts.
- [x] Use SQL `ON CONFLICT` for concurrent translation upsert.
- [x] Route all seller FBA writes through the receipt executor.
- [x] Persist append-only seller lifecycle/moderation events with truthful command or
  legacy-snapshot provenance and bounded newest-first timeline reads.
- [x] Commit onboarding review, suspension, and reactivation state, immutable event,
  completed receipt, and normalized response snapshot in one owner transaction.
- [x] Prove completed-receipt replay does not append another lifecycle event and event
  persistence failure rolls back state plus the pending receipt.

### Seller FFA completed

- [x] Publish module-owned seller admin core/model/transport/i18n/Leptos package.
- [x] Implement native and GraphQL source workflows over the same ports/envelope.
- [x] Use canonical request effective locale and return `resolved_locale`.
- [x] Preserve idempotency key for retries and forbid implicit transport fallback.

### Seller remaining

- [ ] Extend atomic immutable event production to create, profile update, onboarding
  submit, and member commands.
- [ ] Backfill/remove seller compatibility snapshots without fabricating attribution.
- [ ] Publish seller events through the transactional outbox.
- [ ] Add lifecycle/moderation history to native and GraphQL seller FFA workflows.
- [ ] Add normalized verification facts and KYC provider SPI without raw payloads.
- [ ] Compile seller/core/GraphQL/admin packages, apply clean/upgraded PostgreSQL
  migrations, and execute locale, replay, tenant, contention, rollback, mounted FFA,
  and remote-profile evidence.

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
- [x] Persist append-only listing events with typed provenance and bounded newest-first
  timeline reads.
- [x] Route all eight listing commands through atomic owner state/terms + event +
  receipt executors.
- [x] Check completed receipt replay before provider reads for create, publish, and
  reactivate, then re-check admission after provider preflight.
- [x] Require actor/effective locale for command events.
- [x] Keep `MarketplaceListingService` read-only; direct write bypasses are removed.
- [x] Import legacy approval/suspension notes as `legacy_snapshot` events with null
  actor/locale and source-column metadata.
- [x] Remove mutable `approval_note` and `suspension_reason` from final entity, DTO,
  write paths, and post-cutover schema.
- [x] Register listing in modules, distribution, and server as opt-in backend owner.
- [x] Restore evented module registration and FBA command routing after parallel source
  drift.
- [x] Define the sealed, typed nine-event `MarketplaceListingEvent` family and preserve
  it through transactional outbox relay and Iggy transport.
- [x] Publish one external contract event from receipt completion in the same owner
  transaction as state/terms, internal event, and completed receipt.
- [x] Keep moderation notes, reasons, arbitrary metadata, and imported legacy snapshots
  out of the external contract payload and live relay path.

### Listing FFA source completed

- [x] Publish `rustok-marketplace-listing-admin` with framework-neutral models/core,
  explicit transport facade, English/Russian catalogs, and a thin Leptos adapter.
- [x] Model listing directory/detail, current terms, immutable history, and all eight
  listing commands.
- [x] Preserve the same command/idempotency key for explicit retry.
- [x] Render `legacy_snapshot` events as unknown attribution rather than command facts.
- [x] Require a request-scoped host runtime with typed listing ports, authorization,
  and canonical `PortContext` construction for native execution.
- [x] Fail the declared GraphQL profile closed while listing GraphQL roots are absent;
  do not silently fall back to native or fabricate schema operations.
- [x] Register the module-owned admin package and locale path in the listing manifest.
- [x] Register the nested admin crate in workspace/admin hydrate and SSR feature graphs.
- [x] Add platform `marketplace_listings` permissions and module-owned workflow mapping.

### Listing remaining

- [ ] Publish listing GraphQL roots over the same typed ports and replace the
  declared-unmounted FFA adapter.
- [ ] Provide authenticated request-scoped native runtime composition in admin hosts.
- [ ] Add product matching/approval before automated EAN/GTIN matching,
  deduplication, or buy-box ranking.
- [ ] Compile listing/root/provider/admin contracts and execute clean/upgraded
  migrations, replay, provider-preflight races, locale/provenance constraints, tenant
  isolation, PostgreSQL event/outbox atomicity, contention, rollback, relay, restart,
  and mounted transport evidence.

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
- [x] `node scripts/verify/verify-marketplace-seller-events.mjs`
- [ ] `node scripts/verify/verify-marketplace-listing-event-contract.mjs`
- [ ] `node scripts/verify/verify-marketplace-listing-provenance-cutover.mjs`
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`
- [ ] `cargo xtask module validate marketplace`
- [ ] `cargo xtask module validate marketplace_seller`
- [ ] `cargo xtask module validate marketplace_listing`

### Compile/tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-marketplace --lib`
- [x] `cargo check -p rustok-marketplace-seller --all-targets`
- [x] `cargo test -p rustok-marketplace-seller`
- [ ] `cargo check -p rustok-marketplace-seller-admin --all-features`
- [ ] `cargo check -p rustok-marketplace-listing --lib`
- [ ] `cargo test -p rustok-marketplace-listing`
- [ ] `cargo check -p rustok-marketplace-listing-admin --all-features`
- [ ] `cargo check -p rustok-server --features mod-marketplace`
- [ ] Targeted checkout, return-completion, payment, remaining seller/listing lifecycle,
  localization, outbox replay/rollback, recovery, and tenant-isolation tests.

### Database/runtime

- [ ] Apply clean/upgraded SQLite/PostgreSQL and rollback/reapply paths, respecting the
  intentionally irreversible listing provenance cutover.
- [ ] Execute receipt/event/outbox/provider-operation contention and restart scenarios.
- [ ] Execute seller/listing tenant isolation and cross-locale/provenance scenarios.
- [ ] Prove declared routers and module-owned UI packages are mounted.
- [ ] Exercise authenticated checkout, recovery, seller admin, listing admin,
  reconciliation, and replay.
- [ ] Retain remote-profile and real payment/provider evidence.

## Immediate execution order

1. [x] Complete durable return-completion admission and operator recovery source.
2. [x] Create Marketplace root, seller owner, and seller FFA source.
3. [x] Add seller receipts and exact-locale multilingual storage.
4. [x] Create listing owner with terms, receipts, eligibility, and opt-in composition.
5. [x] Add complete listing lifecycle events and remove direct write bypasses.
6. [x] Backfill truthful legacy listing snapshots and remove mutable note columns.
7. [x] Publish the initial module-owned listing FFA source package.
8. [x] Define and atomically publish the sealed listing transactional outbox events.
9. [x] Add listing permissions and workspace/admin feature registration.
10. [x] Add immutable seller lifecycle/moderation event storage and bounded timeline reads.
11. [x] Route onboarding review, suspension, and reactivation through atomic seller
    state + event + receipt completion.
12. [ ] Extend seller event production to create/profile/onboarding-submit/member commands.
13. [ ] Add seller event history to native and GraphQL FFA transports.
14. [ ] Mount authenticated request-scoped listing native composition.
15. [ ] Publish listing GraphQL roots and replace the declared-unmounted adapter.
16. [ ] Run static verifiers and fix remaining source drift.
17. [ ] Compile remaining commerce/payment/Marketplace packages and server features.
18. [ ] Apply clean/upgraded migrations and targeted regression tests.
19. [ ] Run contention, restart, kill-point, tenant, locale, provenance, outbox, and
    mounted transport scenarios.
20. [ ] Introduce seller allocations, commission snapshots, double-entry ledger, and
    payout journals in that order.
21. [ ] Execute production-like payment provider and mounted worker evidence.
22. [ ] Reassess FBA/FFA promotion strictly from retained evidence.

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
