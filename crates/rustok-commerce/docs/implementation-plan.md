# RusToK ecommerce implementation plan

Last reviewed: 2026-07-22

## Source of truth

This file is the only human-maintained source of truth for ecommerce execution
order, completion marks, verification state, and promotion gates.

Rules:

- `[x]` means source or retained execution evidence exists in `main` or in the
  implementation branch that updates this plan.
- `[ ]` means implementation or required evidence is still missing.
- Source implementation and runtime verification are separate tasks.
- Local owner plans and FBA registries may contain owner detail but must not
  contradict this plan.
- No FBA or FFA status is promoted from source inspection alone.
- Newly discovered work is recorded here before or with implementation.
- Legacy migrations must not invent actor, locale, provider, or financial facts.
- A broad invariant must be reopened when one production path still violates it.

`rustok-commerce` owns cross-domain ecommerce orchestration. Product, cart,
customer, region, pricing, inventory, order, payment, fulfillment, tax, promotion,
and market/store remain owner bounded contexts. Marketplace persistence belongs to
the explicit `rustok-marketplace-*` family and must never be folded into
`rustok-commerce`.

## Current boundary

- Ecommerce audit gate: `reopened_p0`.
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
- Marketplace financial source: `reversal_recovery_and_seller_balance_transfer_v3_source_ready_unvalidated`.
- Marketplace production gate: `closed` until compiled contracts, clean/upgraded
  migrations, tenant isolation, contention, restart, mounted transports, remote
  profiles, and financial reconciliation evidence are retained.

## Audit 2026-07-22: reopened P0 work

The following items were rechecked against current `main` after the staged checkout,
payment webhook, marketplace allocation, commission, and ledger source waves.

- [x] Reinspect the current checkout, compensation, order port, payment boundary,
  marketplace economics checkpoint, and master implementation plan.
- [x] Confirm that `CheckoutCompletionPort` still ignores `cart_id` and
  `shipping_option_id`, creates and confirms an order through separate calls, and
  returns `order.checkout_result_projection_unavailable` for recovery reads.
- [x] Confirm that staged checkout and compensation still construct foreign
  `OrderService`, `PaymentService`, and `FulfillmentService` instances directly.
- [x] Confirm that checkout order creation and compensation query the owner-owned
  `orders` table directly through JSON metadata instead of a typed order owner port.
- [x] Confirm that multiple ecommerce port mappers still place raw database/internal
  error text into public `PortError.message`.
- [x] Confirm that checkout/order/payment/fulfillment orchestration still relies on
  string lifecycle matching in critical paths.
- [x] Confirm that the marketplace economics checkpoint migration omitted MySQL
  integrity/immutability guards and left the PostgreSQL trigger function behind on
  rollback.
- [x] Harden the checkpoint migration source with PostgreSQL/SQLite/MySQL guard
  parity, fully immutable update behavior, and explicit backend cleanup before down.
- [x] Make economics checkpoint source admission concurrency-safe: after a failed
  insert, roll back the possibly aborted transaction, adopt the committed row when
  evidence is identical, and classify different evidence as a typed conflict rather
  than exposing a backend unique-violation error.
- [x] Add focused SQLite source coverage for concurrent identical adoption and
  concurrent conflicting evidence classification.
- [ ] Execute the checkpoint concurrency tests and retain PostgreSQL/MySQL contention
  evidence in addition to the SQLite source coverage.
- [ ] Run clean/upgraded/down/reapply checkpoint migration tests on SQLite,
  PostgreSQL, and MySQL; retain evidence.
- [ ] Replace cross-owner order metadata/SQL identity lookup with an owner-owned
  checkout operation identity and typed read/command port.
- [ ] Complete `CheckoutCompletionPort` idempotency and recovery semantics, including
  lookup by checkout operation and cart.
- [ ] Replace direct foreign owner service construction in staged checkout and
  compensation with typed owner ports or explicit owner-provided local adapters.
- [ ] Remove raw DB/provider/internal text from all public ecommerce port errors and
  retain internal structured logs with correlation identity.
- [ ] Propagate typed lifecycle statuses through owner ports and remove string status
  matching from critical checkout, compensation, order, payment, and fulfillment paths.

## FBA/FFA architecture invariants

- [x] Keep owner persistence, lifecycle policy, receipts, events, and provider policy
  inside owner modules.
- [ ] Use typed FBA ports rather than foreign entities, direct foreign service
  construction, or cross-module DB access on every production orchestration path.
- [x] Carry tenant, actor, effective locale, channel, correlation, deadline, and
  idempotency context across published owner calls.
- [x] Keep in-process providers behind the same contracts expected by remote adapters.
- [x] Build FFA as module-owned core/model/transport/i18n/thin-UI packages; hosts only
  compose them.
- [x] Require explicit native/GraphQL transport selection; silent fallback is
  forbidden unless explicitly contracted and verified.
- [x] Use `core_only -> core_transport -> core_transport_ui`.
- [ ] Keep provider raw payloads, signatures, SQL errors, SDK errors, and internal
  invariant details out of owner persistence and public errors on every ecommerce port.
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
- [ ] Resolve cart, product, pricing, inventory, order, payment, and fulfillment only
  through typed owner boundaries; remove the remaining direct service and SQL paths.
- [x] Persist immutable plans, operation identity, hashes, lease, stages, errors, and
  owner ids.
- [x] Keep the checkout inventory reservation entity aligned with the adopted order-line
  column introduced by the checkout lifecycle migration.
- [x] Resume persisted stages and adopt already committed owner outcomes where owner
  identity is available.
- [x] Prevent a second active checkout for the same cart.
- [x] Provide durable compensation state and block provider execution during open
  reconciliation.
- [x] Persist typed marketplace cart/checkout snapshots and fail closed when marketplace
  identity or economics are missing.
- [x] Run marketplace allocation and commission assessment before payment capture.
- [x] Persist a lease-bound allocation/commission economics checkpoint and adopt it on replay.
- [x] Post marketplace ledger after capture through a durable financial operation and gate
  fulfillment on saved ledger evidence.
- [x] Add backend-parity source guards and reversible cleanup for the marketplace
  economics checkpoint migration.
- [x] Add concurrent economics-checkpoint admission/adoption and typed conflict source
  coverage without promoting runtime evidence.
- [ ] Move checkout order identity from JSON metadata into owner-owned typed columns or
  an owner operation table with tenant-safe uniqueness.
- [ ] Make order creation plus placement one idempotent owner command and expose durable
  checkout-result reads by operation/cart.
- [ ] Remove direct `orders` table reads from checkout creation and compensation.
- [ ] Remove direct `OrderService`, `PaymentService`, and `FulfillmentService`
  construction from commerce orchestration.
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
- [x] Remove superseded REST/GraphQL return-completion helper paths after both
  transports moved to `ReturnCompletionOrchestrationService`.
- [ ] Apply return-completion migrations on clean/upgraded SQLite/PostgreSQL.
- [ ] Execute replay, conflict, admission/claim contention, lease expiry, process-exit,
  restart, and reconciliation-resolution evidence.

## Marketplace Family

### Naming and composition

- [x] Use mandatory `rustok-marketplace-*` crate names and `marketplace_*` slugs.
- [x] Publish `rustok-marketplace` as a composition/orchestration root with no owner
  tables.
- [x] Publish seller, listing, allocation, commission, ledger, and payout as separate
  owner modules.
- [x] Keep Marketplace modules opt-in and outside default module/server sets.
- [x] Keep catalog, prices, stock, orders, payments, and generic orchestration in their
  existing owner modules.
- [x] Register owner migrations through the composed `MigrationSource` graph without
  cross-owner foreign keys.

### Marketplace root

- [x] Compose seller directory over `MarketplaceSellerReadPort`.
- [x] Compose listing directory and eligibility over `MarketplaceListingReadPort`.
- [x] Keep root consumers free of SeaORM, owner entities, and owner DB access.
- [x] Compose order commission posting and financial reversals over typed commission and
  ledger ports with deterministic child idempotency keys.
- [x] Advertise ledger v3 seller balance transfer capability without adding root persistence.
- [ ] Add deterministic multi-order payout orchestration over per-order ledger transfers.
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
- [x] Commit seller creation, profile update, onboarding submit, member add, and member
  update state, locale-bound immutable event, completed receipt, and normalized response
  snapshot in one owner transaction.
- [x] Publish every completed live seller/member contract through the transactional
  outbox before receipt completion and transaction commit.
- [x] Prove completed-receipt replay does not append another lifecycle/member event and
  event persistence failure rolls back state plus the pending receipt.

### Seller FFA completed

- [x] Publish module-owned seller admin core/model/transport/i18n/Leptos package.
- [x] Implement native and GraphQL source workflows over the same ports/envelope.
- [x] Use canonical request effective locale and return `resolved_locale`.
- [x] Preserve idempotency key for retries and forbid implicit transport fallback.
- [x] Add bounded lifecycle/member event history models, native server transport,
  GraphQL adapter, and timeline UI over the owner read port.

### Seller remaining

- [ ] Backfill/remove seller compatibility snapshots without fabricating attribution.
- [ ] Add normalized verification facts and KYC provider SPI without raw payloads.
- [ ] Compile seller/core/GraphQL/admin packages, apply clean/upgraded PostgreSQL
  migrations, and execute locale, replay, tenant, contention, rollback, outbox,
  mounted FFA, and remote-profile evidence.

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

- [x] Own immutable order-line allocations without duplicating customer order aggregates.
- [x] Snapshot seller, listing, commission result, fulfillment ownership, and monetary
  allocation at checkout.
- [x] Prevent one seller lifecycle operation from mutating another seller allocation.
- [x] Create versioned deterministic commission policy owner.
- [x] Create immutable double-entry ledger before balances or payouts.
- [x] Derive pending, available, reserved, paid, and negative seller balances from immutable
  seller-payable entries.
- [x] Add append-only refund/chargeback reversals with exact original-entry links and
  cumulative capacity.
- [x] Add payment-owner processed-event observers, durable reversal inbox/recovery, safe
  operator transports, and a durable historical adaptation-failure journal.
- [x] Add append-only pending release, reserve hold/release, payout settlement/reversal
  ledger transfers with exact reference-entry lineage and cumulative capacity.
- [x] Keep PSP split-payment optional; internal allocation/ledger correctness does not
  depend on a PSP.
- [x] Add payout scheduling owner and exclusive ledger-entry assignment.
- [ ] Add payout provider accounts, operation journal, verified webhook inbox, transfer
  execution, lookup recovery, and deterministic multi-order settlement orchestration.
- [ ] Add accounting/vendor surfaces and retained contention/reconciliation evidence.

## Payment workstream

- [x] Keep collections, payments, refunds, provider-operation journals, and webhook
  inbox state in payment.
- [x] Publish typed payment collection ports and idempotent refund identity.
- [x] Guard provider operations through the provider registry with CAS journals and
  explicit reconciliation outcomes.
- [x] Route uncertain external outcomes to reconciliation and forbid auto-reclaim.
- [x] Keep refund reservation validation in the idempotent
  `PaymentRefundCreationService` and remove the superseded `PaymentService` duplicate.
- [x] Add tenant-scoped Stripe source and deployment-owned secret resolution.
- [x] Mount verified webhook ingress and persist only normalized immutable facts.
- [x] Recover received/failed/expired events; isolate dead letters and require
  operator-only replay.
- [x] Normalize `refund.completed` and `chargeback.completed`, run host-composed observers
  only after payment owner application, and mark provider events processed only after observers
  succeed.
- [x] Keep marketplace reversal consumers free of raw provider payloads and signatures.
- [ ] Detect marketplace-associated reversal events that omit required typed marketplace facts
  and route them to durable operator review.
- [ ] Execute production-like Stripe, real signature, redelivery, restart, replica,
  degraded, reconciliation, observer replay, and operator evidence.
- [ ] Prove adapters never own payment/refund lifecycle state.
- [ ] Remove raw database/provider error strings from payment-facing public port errors.

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
- [ ] `cargo xtask module validate order`
- [ ] `cargo xtask module validate payment`
- [ ] `cargo xtask module validate marketplace`
- [ ] `cargo xtask module validate marketplace_seller`
- [ ] `cargo xtask module validate marketplace_listing`
- [ ] Inspect marketplace ledger v3 and seller-balance-transfer v1 source guards.
- [ ] Add a static guard that forbids `FROM orders`, `OrderService::new`,
  `PaymentService::new`, and `FulfillmentService::new` in commerce orchestration after
  the owner-port cutover.
- [ ] Add a static guard for public `PortError` construction from raw `DbErr`/SDK errors.

### Compile/tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo test -p rustok-commerce --test checkout_marketplace_economics_checkpoint`
- [ ] `cargo check -p rustok-order --all-features`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-marketplace --lib`
- [ ] `cargo check -p rustok-marketplace-ledger --all-targets`
- [ ] `cargo test -p rustok-marketplace-ledger`
- [x] `cargo check -p rustok-marketplace-seller --all-targets`
- [x] `cargo test -p rustok-marketplace-seller`
- [ ] `cargo check -p rustok-marketplace-seller-admin --all-features`
- [ ] `cargo check -p rustok-marketplace-listing --lib`
- [ ] `cargo test -p rustok-marketplace-listing`
- [ ] `cargo check -p rustok-marketplace-listing-admin --all-features`
- [ ] `cargo check -p rustok-server --features mod-marketplace`
- [ ] Targeted checkout, order completion, compensation, return-completion, payment,
  marketplace financial recovery, seller balance transfer, remaining seller/listing
  lifecycle, localization, outbox replay/rollback, and tenant-isolation tests.

### Database/runtime

- [ ] Apply clean/upgraded SQLite/PostgreSQL/MySQL and rollback/reapply paths, respecting the
  intentionally irreversible listing provenance cutover.
- [ ] Specifically prove marketplace economics checkpoint identity, amount,
  immutability, tenant/order linkage, and cleanup on all supported backends.
- [ ] Execute receipt/event/outbox/provider-operation/checkpoint contention and restart scenarios.
- [ ] Execute seller/listing tenant isolation and cross-locale/provenance scenarios.
- [ ] Execute reversal observer/inbox/adaptation recovery and safe operator scenarios.
- [ ] Execute seller balance transfer replay, duplicate source, cumulative reference capacity,
  concurrent admission, projection rebuild, and append-only trigger scenarios.
- [ ] Prove declared routers and module-owned UI packages are mounted.
- [ ] Exercise authenticated checkout, recovery, compensation, seller admin, listing admin,
  reconciliation, and replay.
- [ ] Retain remote-profile and real payment/provider evidence.

## Immediate execution order

1. [x] Reaudit current checkout/order/payment/marketplace source and reopen false-complete P0s.
2. [x] Harden marketplace economics checkpoint migration source for backend parity and rollback.
3. [x] Add economics checkpoint concurrent insert adoption and exact conflict classification source.
4. [ ] Add owner-owned checkout operation identity to order storage and migration.
5. [ ] Replace JSON metadata order lookup and direct `orders` SQL with typed order reads.
6. [ ] Complete idempotent `CheckoutCompletionPort` create/place/read semantics.
7. [ ] Cut staged checkout order creation/confirmation over to the completed owner port.
8. [ ] Cut compensation over to typed order/payment owner ports and remove foreign services.
9. [ ] Cut payment and fulfillment stages over to explicit owner adapters/ports.
10. [ ] Remove raw public ecommerce port error leakage and add correlation-safe logging.
11. [ ] Introduce typed lifecycle status snapshots and remove critical string matching.
12. [ ] Run checkout admission, duplicate request, kill-point, restart, and contention evidence.
13. [ ] Run checkpoint clean/upgraded/down/reapply and contention evidence on all supported databases.
14. [ ] Mount authenticated request-scoped listing native composition.
15. [ ] Publish listing GraphQL roots and replace the declared-unmounted adapter.
16. [ ] Add payout provider journal, webhook inbox, multi-order settlement orchestration, and
    reconciliation surfaces.
17. [ ] Run static verifiers and fix remaining source drift.
18. [ ] Compile remaining commerce/order/payment/Marketplace packages and server features.
19. [ ] Apply clean/upgraded migrations and targeted regression tests.
20. [ ] Run contention, restart, kill-point, tenant, locale, provenance, outbox, ledger
    transfer, and mounted transport scenarios.
21. [ ] Execute production-like payment and payout provider evidence.
22. [ ] Reassess FBA/FFA promotion strictly from retained evidence.

## Completed source waves retained for history

- [x] Complete durable return-completion admission and operator recovery source.
- [x] Create Marketplace root, seller owner, and seller FFA source.
- [x] Add seller receipts and exact-locale multilingual storage.
- [x] Create listing owner with terms, receipts, eligibility, and opt-in composition.
- [x] Add complete listing lifecycle events and remove direct write bypasses.
- [x] Backfill truthful legacy listing snapshots and remove mutable note columns.
- [x] Publish the initial module-owned listing FFA source package.
- [x] Define and atomically publish the sealed listing transactional outbox events.
- [x] Add listing permissions and workspace/admin feature registration.
- [x] Add immutable seller lifecycle/moderation event storage and bounded timeline reads.
- [x] Route onboarding review, suspension, and reactivation through atomic seller
  state + event + receipt completion.
- [x] Add typed marketplace allocation, commission, post-capture ledger, reversal recovery,
  adaptation-failure recovery, seller balance projections, and bucket-transfer primitives.
- [x] Extend seller event production to create/profile/onboarding-submit/member commands.
- [x] Add seller event history to native and GraphQL FFA transports.

## Change rules

1. Update this file with every completed or newly discovered ecommerce task.
2. Keep owner plans, registries, manifests, guards, and this plan aligned.
3. Owner modules retain policy, persistence, receipts/events, and commands.
4. Family roots and hosts only compose typed ports and FFA packages.
5. Do not invent legacy actor, locale, provider, or financial facts during migration.
6. Update `docs/modules/registry.md` only when an FFA/FBA status changes.
7. Marketplace names must preserve `rustok-marketplace-*` / `marketplace_*` identity.
8. Do not restore a `[x]` for an audited invariant until every production path and the
   required retained evidence satisfy it.

## References

- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
- [Marketplace root plan](../../rustok-marketplace/docs/implementation-plan.md)
- [Marketplace root FBA registry](../../rustok-marketplace/contracts/marketplace-fba-registry.json)
- [Marketplace ledger FBA registry](../../rustok-marketplace-ledger/contracts/marketplace-ledger-fba-registry.json)
- [Seller balance transfer contract](../../rustok-marketplace-ledger/contracts/seller-balance-transfer-v1.json)
- [Marketplace seller plan](../../rustok-marketplace-seller/docs/implementation-plan.md)
- [Marketplace listing plan](../../rustok-marketplace-listing/docs/implementation-plan.md)
