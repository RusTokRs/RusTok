# Marketplace family implementation plan

Last reviewed: 2026-07-21

## Status

- Family source status: `in_progress`.
- FBA status: `in_progress`.
- FFA status: `in_progress`.
- Runtime integration status: `partial`.
- Migration composition status: `partial`.
- Retained validation evidence: `not_current`.
- Production promotion gate: `closed`.

The maintainer explicitly chose to merge the current source slices before running a
consolidated test pass. A checked source item means that its implementation is present
on `main`; it does not imply that locked compilation, composed migrations, PostgreSQL
contention, mounted transports, or remote-provider evidence are current.

## Source slices merged to `main`

- [x] Seller event/outbox/history composite.
- [x] Marketplace allocation owner.
- [x] Versioned commission owner.
- [x] Immutable double-entry ledger owner.
- [x] Root commission-to-ledger orchestration source.
- [x] Payout scheduling owner.
- [x] Checkout marketplace allocation stage source.
- [x] Isolated cross-domain moderation owner and replay-safe case service.

## Architecture contract

- [x] Use `rustok-marketplace-*` crate names and `marketplace_*` module slugs.
- [x] Keep the family root as composition and orchestration only.
- [x] Keep seller, listing, allocation, commission, ledger, and payout persistence in
  separate owner crates.
- [x] Keep the customer order authoritative in `rustok-order`; marketplace owners store
  immutable allocation and financial projections rather than a second order lifecycle.
- [x] Communicate across owners through typed FBA ports carrying `PortContext`.
- [x] Require write idempotency keys and stable request hashes in owner commands.
- [x] Use currency minor units and checked integer arithmetic in marketplace finance.
- [x] Avoid cross-owner database foreign keys.
- [x] Preserve unknown legacy provenance instead of fabricating actor, locale, provider,
  or financial attribution.
- [ ] Remove every remaining marketplace identity dependency on arbitrary metadata.
- [ ] Ensure every transport resolves owner ports from runtime composition instead of
  constructing concrete owner services.
- [ ] Ensure every owner receives infrastructure transports such as the transactional
  event bus through host composition.

## Seller owner

### Source completed

- [x] Own seller, translation, member, receipt, and immutable event storage.
- [x] Bind effective locale into seller and member command identity.
- [x] Persist seller lifecycle events atomically with state and completed receipts.
- [x] Persist member add/update events atomically with member state and receipts.
- [x] Publish sealed seller event contracts through the transactional outbox path.
- [x] Backfill legacy onboarding and suspension prose as explicit legacy snapshots.
- [x] Project current seller prose from the immutable event timeline.
- [x] Publish bounded seller event-history models, native transport, GraphQL transport,
  and Leptos timeline components.

### Remaining

- [ ] Inject `TransactionalEventBus` from host composition; owner receipt code must not
  construct `OutboxTransport` directly.
- [ ] Resolve `MarketplaceSellerReadPort` from runtime composition in admin transports;
  transports must not instantiate `MarketplaceSellerService` directly.
- [ ] Remove transitional mutable prose columns after clean and upgraded migration
  evidence is retained.
- [ ] Add normalized KYC/verification facts and a provider SPI without raw provider
  payload persistence.
- [ ] Add seller-scoped vendor roles, invitations, and capability enforcement.
- [ ] Retain native/GraphQL parity and PostgreSQL replay/contention evidence.

## Listing owner

### Source completed

- [x] Own seller/master-product/master-variant/market/channel identity and versioned
  listing terms.
- [x] Persist durable command receipts and append-only listing events.
- [x] Route listing writes through atomic state/terms, internal event, receipt, and
  transactional outbox publication.
- [x] Preserve replay before seller and product provider reads.
- [x] Backfill legacy moderation prose with truthful unknown provenance.
- [x] Remove mutable approval and suspension prose from final listing storage.
- [x] Publish listing admin source boundaries and permissions.

### Remaining

- [ ] Mount authenticated native provider composition and real GraphQL roots.
- [ ] Add the typed checkout-resolution port that validates seller state, listing state,
  terms version, market/channel, product identity, currency, quantity, inventory, and
  fulfillment references in one snapshot.
- [ ] Add moderation subject application through `ModerationSubjectCommandPort`.
- [ ] Retain clean/upgraded migrations, outbox relay/restart, contention, and transport
  evidence.

## Checkout and allocation

### Source completed

- [x] Add `rustok-marketplace-allocation` as the immutable order-line allocation owner.
- [x] Enforce one tenant-scoped allocation per order-line identity.
- [x] Persist seller, listing, product, variant, terms version, pricing, inventory,
  fulfillment, quantity, currency, and monetary snapshots.
- [x] Allocate multi-line commands atomically with durable receipt replay.
- [x] Publish tenant-scoped allocation read and command ports.
- [x] Add `CheckoutMarketplaceAllocationStage` with deterministic child key
  `checkout:{operation_id}:marketplace-allocation:v1`.
- [x] Keep ordinary non-marketplace orders as a no-op at the allocation stage.

### Remaining critical path

- [ ] Add marketplace-owned typed checkout line snapshots.
- [ ] Replace `order_line.metadata.marketplace` extraction with the typed snapshot owner.
- [ ] Wire `CheckoutMarketplaceAllocationStage::allocate_if_present` into the real
  pipeline after `CheckoutPaymentReadyState` validation and before capture.
- [ ] Compose `MarketplaceAllocationCommandPort` in the runtime checkout pipeline.
- [ ] Persist the allocation result/checkpoint in the durable checkout operation.
- [ ] Add allocation cancellation for payment failure, checkout compensation, order
  cancellation, and line cancellation.
- [ ] Publish allocation created/cancelled events transactionally.
- [ ] Register allocation migrations in the composed migrator.
- [ ] Retain concurrent one-allocation-per-line PostgreSQL evidence.

## Commission

### Source completed

- [x] Add append-only commission rule versions.
- [x] Select rules deterministically by listing, seller, global specificity, priority,
  version, effective timestamp, and stable ID.
- [x] Calculate percentage and fixed commission in minor units with checked arithmetic.
- [x] Persist immutable per-allocation assessment snapshots.
- [x] Enforce one assessment per allocation and atomic order-wide assessment batches.
- [x] Replay completed receipts before allocation provider reads.
- [x] Reject cancelled allocations before receipt admission.
- [x] Publish typed commission read and command ports.

### Remaining

- [ ] Run commission assessment after allocation and before payment capture.
- [ ] Extend rule scope for category, product type, seller tier, market, and channel.
- [ ] Add explicit commission-base policy for item, shipping, tax, discount, minimum,
  and maximum components.
- [ ] Add commission assessment cancellation and reversal lifecycle.
- [ ] Publish commission events with state, receipt, and outbox in one transaction.
- [ ] Add commission rule management admin surfaces.
- [ ] Register commission migrations in the composed migrator.
- [ ] Retain rule selection, replay, and contention evidence on PostgreSQL.

## Ledger and financial orchestration

### Source completed

- [x] Add immutable ledger transactions and entries.
- [x] Post balanced order commission batches.
- [x] Debit marketplace clearing and credit platform commission revenue and seller
  payable accounts.
- [x] Validate one currency per batch and debit/credit equality.
- [x] Replay completed receipts before commission provider reads.
- [x] Publish order-ledger and seller-payable read projections.
- [x] Add root orchestration that derives stable commission and ledger child keys.
- [x] Validate commission aggregates against ledger totals.

### Remaining critical path

- [ ] Split marketplace finance timing: commission economics before capture; ledger
  posting only after payment capture/order paid.
- [ ] Add durable `marketplace_financial_operations` stage journal, leases, retries,
  safe errors, recovery worker, and operator review state.
- [ ] Trigger ledger posting from a deduplicated paid-event inbox.
- [ ] Add append-only reversal transactions for refunds, chargebacks, adjustments,
  payout settlement, payout reversal, reserve hold, and reserve release.
- [ ] Add seller balance projections for pending, available, reserved, paid, and
  negative amounts, rebuildable from ledger entries.
- [ ] Hold fulfillment when financial posting is incomplete and release it after
  successful posting according to policy.
- [ ] Register ledger migrations in the composed migrator.
- [ ] Retain duplicate-paid-event, balancing, recovery, and concurrent posting evidence.

## Payout

### Source completed

- [x] Add the payout scheduling owner.
- [x] Assign seller-payable ledger entries exclusively to one payout batch.
- [x] Validate seller, currency, positive amount, account, and direction.
- [x] Commit payout header, items, totals, and completed receipt atomically.
- [x] Publish payout reads by payout and seller.
- [x] Register payout in distribution and marketplace family composition.

### Remaining production lifecycle

- [ ] Add seller payout accounts and normalized provider onboarding status.
- [ ] Add `MarketplacePayoutProvider` SPI.
- [ ] Add the first provider adapter and secret resolution through the platform secret
  boundary.
- [ ] Add provider operation journal and verified webhook inbox.
- [ ] Implement scheduled, processing, submitted, paid, failed, cancelled,
  reversal-pending, reversed, and operator-review transitions with CAS.
- [ ] Add transfer idempotency, provider lookup recovery, and duplicate webhook replay.
- [ ] Post payout clearing, cash/provider settlement, and reversal ledger transactions.
- [ ] Add payout eligibility policy for delivery, return window, disputes, reserves,
  minimum amount, and seller risk.
- [ ] Add finance reconciliation and seller payout UI.
- [ ] Register payout migrations in the composed migrator.
- [ ] Retain provider crash/retry and concurrent ledger-entry assignment evidence.

## Moderation integration

### Source completed

- [x] Add the isolated `rustok-moderation` owner crate.
- [x] Add typed subject/scope, report, case, assignment, decision, and subject-application
  contracts.
- [x] Add owner schema for reports, cases, report links, immutable decisions, receipts,
  and events.
- [x] Add receipt-first report, case, assignment, and decision services.
- [x] Add deterministic active-case deduplication and revision compare-and-set.
- [x] Keep moderation from writing foreign owner tables directly.

### Remaining

- [ ] Reconcile `Cargo.lock` after the isolated owner merge.
- [ ] Register moderation in workspace dependency aliases, module catalog, and composed
  migrator without replacing newer global files.
- [ ] Add moderation RBAC resources and runtime composition.
- [ ] Publish moderation lifecycle events through the transactional outbox.
- [ ] Add durable decision-application journal and crash/retry recovery.
- [ ] Add seller and listing `ModerationSubjectCommandPort` adapters.
- [ ] Add policy snapshots, automated assessments, appeals, and sanctions.
- [ ] Add admin queue/case UI and PostgreSQL contention evidence.

## Multi-seller order management

- [ ] Add seller-order projections sourced from order allocations.
- [ ] Scope seller reads and commands to owned allocations only.
- [ ] Support split fulfillment and tracking per seller allocation.
- [ ] Add return/refund attribution to order line, allocation, seller, and commission
  assessment.
- [ ] Add commission and ledger reversal orchestration for partial refunds.
- [ ] Add disputes, evidence, financial holds, and resolution lifecycle.
- [ ] Preserve one customer-facing aggregate order.

## Vendor, admin, and storefront surfaces

- [ ] Add `/vendor/*` actor context and seller-scoped capability checks.
- [ ] Add vendor workspace sections for onboarding, members, listings, inventory,
  pricing, orders, fulfillment, returns, disputes, commission, ledger, payouts, and
  analytics.
- [ ] Add platform finance and reconciliation surfaces.
- [ ] Add buy-box selection using landed price, stock, seller quality, fulfillment
  promise, returns, market/channel, and merchandising policy.
- [ ] Add seller profile, verified purchase review, seller response, messaging, and
  moderation surfaces.
- [ ] Add promotion funding attribution: platform, seller, or shared.

## Migration and runtime composition queue

- [ ] Register allocation, commission, ledger, payout, and moderation owner migrations
  in the current composed migrator.
- [ ] Add workspace dependency aliases where required without reverting concurrent root
  manifest changes.
- [ ] Reconcile the workspace lock after all owner crates are registered.
- [ ] Register runtime providers for allocation, commission, ledger, payout, and
  moderation.
- [ ] Register checkout and paid-event consumers in host composition.
- [ ] Update module implementation-plan and backfill registries with the final composed
  migration order.

## Consolidated maintainer validation queue

No new tests were run for the 2026-07-21 source merge batch.

- [ ] Reconcile `Cargo.lock`.
- [ ] Run formatting for changed marketplace and moderation crates.
- [ ] Run `cargo check` for marketplace owners, commerce, distribution, moderation, and
  the composed migrator.
- [ ] Run owner unit and SQLite service tests.
- [ ] Export and inspect the composed migration plan.
- [ ] Apply clean and upgraded SQLite migrations.
- [ ] Apply clean and upgraded PostgreSQL migrations.
- [ ] Run idempotency conflict and lost-response replay scenarios.
- [ ] Run allocation, commission, ledger, payout, seller, listing, and moderation
  contention scenarios.
- [ ] Run outbox relay/restart and webhook replay scenarios.
- [ ] Run native/GraphQL mounted transport parity.
- [ ] Run embedded and remote FBA timeout/degraded-mode scenarios.
- [ ] Run cross-tenant and cross-seller authorization scenarios.

## Immediate execution order

1. [ ] Register all newly merged owner migrations in the current composed migrator and
   reconcile the workspace lock.
2. [ ] Add typed marketplace checkout snapshots and remove metadata-based identity.
3. [ ] Wire allocation and pre-capture commission stages into the real checkout pipeline.
4. [ ] Add durable post-capture financial operation and paid-event inbox.
5. [ ] Add refund/chargeback ledger reversals and seller balance projections.
6. [ ] Add payout provider journal, webhook inbox, transfer execution, and settlement.
7. [ ] Add seller/listing moderation adapters and decision-application recovery.
8. [ ] Add seller-order, fulfillment, return, refund, and dispute projections.
9. [ ] Build vendor and finance control-room surfaces.
10. [ ] Execute the consolidated validation queue and only then promote readiness.

## Primary source paths

- `crates/rustok-marketplace/src/`
- `crates/rustok-marketplace-seller/`
- `crates/rustok-marketplace-listing/`
- `crates/rustok-marketplace-allocation/`
- `crates/rustok-marketplace-commission/`
- `crates/rustok-marketplace-ledger/`
- `crates/rustok-marketplace-payout/`
- `crates/rustok-commerce/src/services/checkout_marketplace_allocation.rs`
- `crates/rustok-moderation/`
