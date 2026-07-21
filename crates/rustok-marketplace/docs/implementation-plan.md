# Marketplace family implementation plan

Last reviewed: 2026-07-21

## Status

- Family source status: `in_progress`.
- FBA status: `in_progress`.
- FFA status: `in_progress`.
- Runtime integration status: `refund_chargeback_observer_reversal_inbox_worker_rest_graphql_source_ready_unvalidated`.
- Migration composition status: `source_wired_unvalidated`.
- Retained validation evidence: `not_current`.
- Production promotion gate: `closed`.

The maintainer explicitly chose to merge the current source slices before running a
consolidated test pass. A checked source item means that its implementation is present
on `main`; it does not imply that locked compilation, composed migration execution,
PostgreSQL contention, mounted transport execution, or remote-provider evidence are current.

## Source slices merged to `main`

- [x] Seller event/outbox/history composite.
- [x] Marketplace allocation owner.
- [x] Versioned commission owner.
- [x] Immutable double-entry ledger owner.
- [x] Root commission-to-ledger orchestration source.
- [x] Payout scheduling owner.
- [x] Checkout marketplace allocation stage source.
- [x] Isolated cross-domain moderation owner and replay-safe case service.
- [x] Workspace aliases, module catalog, distribution/server feature graph, and composed
  migration source wiring for the Marketplace Family and moderation.
- [x] Cart-owned typed marketplace line identity/economics storage and FBA ports.
- [x] Immutable typed marketplace checkout plan with real pre-capture allocation and
  commission assessment hooks.
- [x] Durable checkout marketplace economics checkpoint with lease-bound admission and
  replay adoption.
- [x] Commerce-owned durable post-capture financial operation with ledger receipt replay
  and fulfillment gating.
- [x] Deduplicated paid-event inbox, paid-order listener, verified provider-event adapter,
  bounded recovery sweep, and operator recovery service source.
- [x] Host-composed marketplace financial runtime, scheduled recovery worker, authenticated
  REST and GraphQL operator transports, and OpenAPI source.
- [x] Append-only refund/chargeback ledger reversals with exact original-entry links,
  cumulative reversal capacity, rebuildable seller balances, and root reversal orchestration.
- [x] Host-observed payment refund/chargeback normalization, durable reversal-event inbox,
  scheduled backfill/recovery, and authenticated REST/GraphQL operator transports.

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
- [x] Stop checkout/allocation from treating arbitrary order-line metadata as marketplace
  identity or financial truth.
- [ ] Remove the remaining compatibility seller projection from cart line metadata after
  delivery-group grouping consumes typed cart marketplace snapshots directly.
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
  fulfillment references before the cart snapshot is written.
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
- [x] Register seller, listing, and allocation `MigrationSource` implementations in the
  composed migrator source list.
- [x] Add cart-owned `cart_line_item_marketplace_snapshots` with seller/listing/product/
  variant/terms identity, currency code/exponent, minor-unit economics, references, and
  owner-local line-item FK.
- [x] Add typed cart snapshot read/write FBA ports.
- [x] Add atomic cart command that writes the line item, localized title, and typed
  marketplace snapshot in one transaction.
- [x] Require exact Decimal-to-minor-unit conversion with no implicit rounding.
- [x] Persist typed marketplace lines in the immutable hashed checkout order plan.
- [x] Fail checkout closed when a seller/legacy marketplace marker has no typed snapshot.
- [x] Remove marketplace/seller identity JSON from order-line metadata before order
  creation.
- [x] Build allocation requests from typed plan snapshots and created order-line indexes,
  not from `order_line.metadata.marketplace`.
- [x] Invoke allocation after `CheckoutPaymentReadyState` and before capture.
- [x] Compose the in-process allocation command port in storefront staged checkout.
- [x] Invoke commission assessment after allocation and before payment capture.
- [x] Compose the commission command port against the same allocation read owner.
- [x] Persist one immutable `checkout_marketplace_economics_checkpoints` row per checkout
  operation with order/plan/currency identity, counts, aggregates, and canonical set hashes.
- [x] Require the current executing checkout lease at `payment_ready` before checkpoint
  admission.
- [x] Adopt a matching checkpoint on retry or completed-response replay without repeating
  allocation or commission owner calls.
- [x] Classify retryable allocation, commission, and checkpoint storage outages as
  `retryable_error` instead of forcing compensation.

### Remaining critical path

- [ ] Add native/GraphQL storefront adapters and UI/storefront commands for the atomic
  marketplace cart-line command.
- [ ] Add a durable receipt for atomic marketplace cart-line command replay and payload
  conflict detection.
- [ ] Synchronize typed snapshot economics when marketplace line quantity, price,
  discounts, or tax change; immutable checkout plan remains the final freeze point.
- [ ] Replace compatibility `seller_id` metadata used by delivery-group grouping with a
  typed join against cart marketplace snapshots.
- [ ] Add allocation cancellation for payment failure, checkout compensation, order
  cancellation, and line cancellation.
- [ ] Publish allocation created/cancelled events transactionally.
- [ ] Retain clean/upgraded migration, checkpoint replay, lost-response, and concurrent
  one-allocation-per-line PostgreSQL evidence.

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
- [x] Register commission `MigrationSource` in the composed migrator source list.
- [x] Add `CheckoutMarketplaceCommissionStage` with deterministic child key
  `checkout:{operation_id}:marketplace-commission:v1`.
- [x] Use stable order creation time as `assessed_at` so retries preserve the owner request
  hash.
- [x] Validate assessment coverage, allocation uniqueness, order-line identity, currency,
  and non-negative economics before capture.
- [x] Bind the exact assessment allocation set and aggregate economics into the durable
  checkout checkpoint.

### Remaining

- [ ] Extend rule scope for category, product type, seller tier, market, and channel.
- [ ] Add explicit commission-base policy for item, shipping, tax, discount, minimum,
  and maximum components.
- [ ] Add commission assessment cancellation and reversal lifecycle.
- [ ] Publish commission events with state, receipt, and outbox in one transaction.
- [ ] Add commission rule management admin surfaces.
- [ ] Retain clean/upgraded migrations, rule selection, replay, and contention evidence on
  PostgreSQL.

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
- [x] Register ledger `MigrationSource` in the composed migrator source list.
- [x] Keep commission economics before capture without posting ledger entries from the
  pre-capture checkout path.
- [x] Add commerce-owned `marketplace_financial_operations` with immutable captured-payment,
  order, plan, currency, idempotency, and request identity.
- [x] Add pending/executing/retryable-error/operator-review/completed states, CAS leases,
  attempt counts, safe error fields, and immutable ledger evidence.
- [x] Post ledger only after a fully captured payment collection using stable
  `checkout:{operation_id}:marketplace-ledger:v1` identity and captured timestamp.
- [x] Reconcile ledger debit/credit totals to the pre-capture economics checkpoint.
- [x] Block marketplace fulfillment until the financial operation is completed with
  `ledger_posted` evidence.
- [x] Compose the commission-backed ledger owner in storefront staged checkout runtime.
- [x] Preserve typed checkpoint storage failures so outages remain retryable after capture.
- [x] Persist invalid ledger responses and non-retryable owner failures as explicit
  `operator_review` before releasing the financial lease.
- [x] Use checkout-operation service identity for the ledger audit actor so direct checkout,
  paid-order events, and verified provider-event replay share the same owner request hash.
- [x] Add immutable `marketplace_paid_event_inbox` normalized facts, tenant/source/event
  deduplication, CAS leases, retryable/operator-review states, and processed-row locking.
- [x] Revalidate checkout operation, immutable plan, order, payment identity, captured time,
  currency, amount, and plan hash before admitting event-driven ledger processing.
- [x] Register a durable `OrderStatusChanged -> paid` handler in `CommerceModule`; ordinary
  and non-marketplace orders remain no-ops.
- [x] Add an adapter for signature-verified, payment-owner-processed `payment.captured`
  provider events without persisting raw webhook payloads.
- [x] Add bounded recovery sweep source for received, retryable-error, and expired-processing
  inbox rows.
- [x] Add bounded operator list/show/reset/retry service source with safe pre-ledger retry
  restrictions.
- [x] Add `MarketplaceFinancialRuntime` and resolve ledger capability from host composition
  across the paid-order listener, scheduled worker, REST, and GraphQL.
- [x] Mount the bounded paid-event recovery worker in the server background-worker lifecycle
  with delayed missed ticks and the shared shutdown signal.
- [x] Add tenant-scoped authenticated REST and GraphQL list/show/retry/sweep surfaces using
  safe projections and `payments:read` / `payments:manage` authorization.
- [x] Register REST routes in commerce OpenAPI and preserve the existing `CommerceQuery`
  type/value construction contract while merging the financial query root.
- [x] Add owner-local append-only refund and chargeback reversal transactions with durable
  receipts, exact links to original entries, stable source identity, and balanced postings.
- [x] Lock original entries before cumulative reversal-capacity checks to prevent concurrent
  over-reversal on databases that support row locking.
- [x] Add pending, available, reserved, paid, and negative seller balance projections that
  rebuild from immutable seller-payable entries.
- [x] Rebuild affected seller balances after initial posting, reversal posting, and receipt
  replay without making the projection authoritative.
- [x] Add root `process_financial_reversal` orchestration with stable
  `<root>:ledger-reversal:v1` child identity and no commission reassessment.
- [x] Publish marketplace ledger v2 and marketplace family v2 contracts plus source tests for
  replay, duplicate source, cumulative capacity, immutable originals, projection rebuild,
  root child-key propagation, and result invariants.
- [x] Extend payment normalized events with validated `chargeback.completed` facts and publish
  a host-composable processed-event observer contract.
- [x] Mount `PaymentObservedDomainEventApplier` in webhook ingress, manual recovery, and the
  scheduled payment recovery worker; payment events become processed only after observers
  succeed.
- [x] Compose `PaymentProviderEventObservers` before worker startup and through HTTP/runtime
  host values without adding a payment-to-commerce dependency.
- [x] Consume only signature-verified refund/chargeback facts after the payment owner stage;
  ordinary non-marketplace events remain no-ops and marketplace code never parses raw payloads.
- [x] Require the optional `marketplace_reversal` extension to carry exact allocation,
  assessment, order-line, seller, commission, seller amount, currency exponent, and event time.
- [x] Derive refund payment collection identity from the authoritative refund owner and require
  chargeback collection identity as a normalized provider fact.
- [x] Convert provider major-unit amounts to minor units exactly and reject implicit rounding or
  a mismatch with the normalized reversal line total.
- [x] Add immutable `marketplace_reversal_event_inbox` facts with provider-event,
  source-event, and reversal-source deduplication, CAS leases, retryable/operator-review states,
  and resulting reversal/ledger evidence.
- [x] Invoke the marketplace root through stable
  `marketplace-reversal-event:{inbox_id}:v1` identity and service actor causation.
- [x] Keep bounded processed-event polling as historical backfill/fallback, then recover
  reversal inbox rows before the existing paid-event sweep.
- [x] Add tenant-scoped REST and GraphQL reversal list/show/retry/sweep surfaces with
  `payments:read` / `payments:manage` authorization and OpenAPI registration.
- [x] Exclude reversal lines, provider metadata, hashes, lease details, raw payloads, and
  signatures from operator projections; allow retry only before reversal/ledger evidence exists.
- [x] Publish the source-only marketplace reversal recovery contract and regression guard.

### Remaining critical path

- [ ] Add durable adaptation-failure tracking for malformed marketplace extensions discovered
  during historical processed-event polling.
- [ ] Add append-only adjustment, payout settlement, payout reversal, reserve hold, reserve
  release, and seller balance bucket-transfer transactions.
- [ ] Retain clean/upgraded migrations, observer failure/replay, normalized-event no-op/conflict,
  reversal inbox replay, duplicate source, exact conversion, lease expiry, worker lifecycle,
  authorization, REST/GraphQL/OpenAPI mounting, PostgreSQL contention, and balance rebuild
  evidence.

## Payout

### Source completed

- [x] Add the payout scheduling owner.
- [x] Assign seller-payable ledger entries exclusively to one payout batch.
- [x] Validate seller, currency, positive amount, account, and direction.
- [x] Commit payout header, items, totals, and completed receipt atomically.
- [x] Publish payout reads by payout and seller.
- [x] Register payout in distribution and marketplace family composition.
- [x] Register payout `MigrationSource` in the composed migrator source list.

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
- [ ] Retain clean/upgraded migrations, provider crash/retry, and concurrent ledger-entry
  assignment evidence.

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
- [x] Register moderation as a workspace dependency alias and optional module in
  `modules.toml`, `rustok-distribution`, and the server feature graph.
- [x] Register moderation `MigrationSource` and dependency descriptors in the composed
  migrator source list.

### Remaining

- [ ] Reconcile `Cargo.lock` after manifest composition changes.
- [ ] Add moderation RBAC resources and request-scoped runtime port composition.
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
- [x] Connect normalized partial refunds and chargebacks to the source-ready ledger reversal
  orchestration through the durable commerce reversal inbox.
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

- [x] Register seller, listing, allocation, commission, ledger, payout, and moderation
  owner migrations in the current composed migrator source list.
- [x] Add workspace dependency aliases without reverting concurrent root manifest
  changes.
- [x] Register the complete Marketplace Family and moderation in the module catalog,
  distribution registry, and server opt-in feature graph.
- [x] Register allocation, commission, and ledger providers in the storefront staged
  checkout runtime.
- [x] Register the commerce-owned marketplace economics checkpoint migration with
  dependencies on checkout plan, allocation, and commission schemas.
- [x] Register the commerce-owned financial operation migration with dependencies on
  captured payment binding, pre-capture checkpoint, and marketplace ledger schemas.
- [x] Register the paid-event inbox migration after financial operation and immutable
  payment provider-event normalized facts.
- [x] Register the paid-order marketplace financial event listener in commerce host
  composition.
- [x] Register `MarketplaceFinancialRuntime` in server shared/module/HTTP/GraphQL
  composition and enrich listener extensions after event transport startup.
- [x] Mount scheduled paid-event recovery and authenticated REST/GraphQL operator
  transports.
- [x] Register marketplace ledger reversal and seller balance migrations through the
  existing ledger `MigrationSource` composition.
- [x] Register `m20260721_000004_create_marketplace_reversal_event_inbox` after immutable
  payment normalized facts and marketplace ledger reversal storage.
- [x] Extend `MarketplaceFinancialRuntime` with the root financial command port and compose
  reversal adapter, inbox, worker, and operator services from the host runtime.
- [x] Mount bounded processed-event adaptation, reversal recovery, REST/GraphQL transports,
  and OpenAPI source in the existing server/commerce composition.
- [x] Mount the processed-event observer registry in payment ingress, manual recovery,
  scheduled payment recovery, host runtime values, and pre-worker startup composition.
- [ ] Reconcile the workspace lock after all owner crates are registered.
- [ ] Register request-scoped runtime providers for payout and moderation; compile-time
  module registration is not sufficient.
- [ ] Update backfill registries with the validated final composed migration order.

## Consolidated maintainer validation queue

No new tests were run for the 2026-07-21 source composition, typed-checkout, checkpoint,
post-capture financial-operation, paid-event recovery, scheduled worker, operator transport,
append-only reversal, seller balance projection, root reversal orchestration, normalized
refund/chargeback observation, reversal inbox recovery, or reversal operator transport batches.

- [ ] Reconcile `Cargo.lock`.
- [ ] Run formatting for changed cart, commerce, marketplace, payment, moderation,
  distribution, server, and migration crates.
- [ ] Run `cargo check` for cart, marketplace owners, payment, commerce, distribution,
  moderation, the server feature graph, and the composed migrator.
- [ ] Run owner unit and SQLite service tests.
- [ ] Export and inspect the composed migration plan.
- [ ] Apply clean and upgraded SQLite migrations.
- [ ] Apply clean and upgraded PostgreSQL migrations.
- [ ] Run typed snapshot currency/exponent, exact minor-unit conversion, atomic add,
  conflict, and checkout fail-closed scenarios.
- [ ] Run checkpoint write, exact replay, conflicting evidence, expired lease, process-exit
  after owner commits, and resume-without-owner-call scenarios.
- [ ] Run financial-operation admission, concurrent claim, lease expiry, ledger lost-response,
  ledger receipt replay, operator-review, and fulfillment-gate scenarios.
- [ ] Run paid-event duplicate replay, same-key/different-facts conflict, paid-order listener
  replay, verified-provider adapter, expired inbox lease, sweep, and operator retry scenarios.
- [ ] Run direct observer success/failure, owner-committed-observer-failed replay, dead-letter
  replay, empty observer registry, and startup-before-payment-worker scenarios.
- [ ] Run refund/chargeback marketplace extension absence as no-op, malformed extension,
  authoritative refund/payment mismatch, exact conversion, and line-total conflict scenarios.
- [ ] Run reversal inbox provider-event, source-event, and reversal-source deduplication,
  same-identity/different-facts conflict, lease expiry, replay, and lost-response scenarios.
- [ ] Run worker startup/shutdown, disabled-background-worker, bounded tick, expired-lease,
  adaptation-before-reversal-before-paid ordering, multi-tenant fairness, and repeated sweeps.
- [ ] Run REST/OpenAPI and GraphQL schema mounting, tenant isolation, payment RBAC,
  safe-projection, list/show/retry, and manual sweep scenarios for paid and reversal inboxes.
- [ ] Run reversal receipt replay, same-key/different-request conflict, duplicate source,
  cumulative over-reversal, immutable original-entry, and corrupted projection rebuild scenarios.
- [ ] Run concurrent PostgreSQL reversal admission against the same original entries.
- [ ] Run root reversal child-key, deadline/causation propagation, no-commission-call, identity,
  transaction binding, and balanced-total scenarios.
- [ ] Run idempotency conflict and lost-response replay scenarios.
- [ ] Run allocation, commission, ledger, payout, seller, listing, and moderation
  contention scenarios.
- [ ] Run outbox relay/restart and webhook replay scenarios.
- [ ] Run native/GraphQL mounted transport parity.
- [ ] Run embedded and remote FBA timeout/degraded-mode scenarios.
- [ ] Run cross-tenant and cross-seller authorization scenarios.

## Immediate execution order

1. [x] Register owner workspace aliases, module feature graph, and composed migration
   sources.
2. [x] Add typed marketplace cart/checkout snapshots and remove metadata-based identity
   from checkout and allocation.
3. [x] Wire marketplace allocation and commission assessment into the real pipeline before
   payment capture.
4. [x] Add durable allocation/commission checkpoint and replay adoption.
5. [x] Add durable direct-checkout post-capture financial operation, ledger posting, and
   fulfillment gate.
6. [x] Add paid-event inbox, paid-order listener, verified provider adapter, bounded sweep,
   and operator recovery service source.
7. [x] Mount scheduled sweep and authenticated REST/GraphQL operator transports.
8. [x] Add refund/chargeback ledger reversals and seller balance projections.
9. [x] Normalize refund/chargeback owner events, mount post-owner observers, and add durable
   reversal recovery.
10. [ ] Add durable historical adaptation-failure tracking.
11. [ ] Add balance bucket transfers, payout provider journal, webhook inbox, transfer
    execution, and settlement.
12. [ ] Add seller/listing moderation adapters and decision-application recovery.
13. [ ] Add seller-order, fulfillment, return, refund, and dispute projections.
14. [ ] Build vendor and finance control-room surfaces.
15. [ ] Execute the consolidated validation queue and only then promote readiness.

## Primary source paths

- `crates/rustok-marketplace/src/`
- `crates/rustok-marketplace-seller/`
- `crates/rustok-marketplace-listing/`
- `crates/rustok-marketplace-allocation/`
- `crates/rustok-marketplace-commission/`
- `crates/rustok-marketplace-ledger/`
- `crates/rustok-marketplace-ledger/src/reversal.rs`
- `crates/rustok-marketplace-ledger/src/balance.rs`
- `crates/rustok-marketplace-ledger/src/migrations/m20260721_000002_add_reversals_and_seller_balances.rs`
- `crates/rustok-marketplace-ledger/tests/reversal_projection_test.rs`
- `crates/rustok-marketplace-payout/`
- `crates/rustok-marketplace/src/financial_orchestration.rs`
- `crates/rustok-marketplace/tests/financial_reversal_orchestration_test.rs`
- `crates/rustok-marketplace/contracts/financial-orchestration-v2.json`
- `crates/rustok-cart/src/entities/cart_line_item_marketplace_snapshot.rs`
- `crates/rustok-cart/src/services/marketplace_snapshot.rs`
- `crates/rustok-cart/src/marketplace_snapshot.rs`
- `crates/rustok-payment/src/services/provider_event_chargeback.rs`
- `crates/rustok-payment/src/services/provider_event_observer.rs`
- `crates/rustok-payment/src/controllers.rs`
- `crates/rustok-payment/src/provider_event_recovery_controller.rs`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`
- `crates/rustok-payment/docs/provider-webhooks.md`
- `crates/rustok-commerce/src/entities/checkout_marketplace_economics_checkpoint.rs`
- `crates/rustok-commerce/src/entities/marketplace_financial_operation.rs`
- `crates/rustok-commerce/src/entities/marketplace_paid_event_inbox.rs`
- `crates/rustok-commerce/src/entities/marketplace_reversal_event_inbox.rs`
- `crates/rustok-commerce/src/migrations/m20260721_000001_create_checkout_marketplace_economics_checkpoints.rs`
- `crates/rustok-commerce/src/migrations/m20260721_000002_create_marketplace_financial_operations.rs`
- `crates/rustok-commerce/src/migrations/m20260721_000003_create_marketplace_paid_event_inbox.rs`
- `crates/rustok-commerce/src/migrations/m20260721_000004_create_marketplace_reversal_event_inbox.rs`
- `crates/rustok-commerce/src/services/checkout_order_plan.rs`
- `crates/rustok-commerce/src/services/checkout_plan_builder.rs`
- `crates/rustok-commerce/src/services/checkout_marketplace_allocation.rs`
- `crates/rustok-commerce/src/services/checkout_marketplace_commission.rs`
- `crates/rustok-commerce/src/services/checkout_marketplace_economics.rs`
- `crates/rustok-commerce/src/services/checkout_marketplace_financial_hardened.rs`
- `crates/rustok-commerce/src/services/marketplace_paid_event_inbox.rs`
- `crates/rustok-commerce/src/services/marketplace_paid_order_financial.rs`
- `crates/rustok-commerce/src/services/marketplace_provider_paid_event_adapter.rs`
- `crates/rustok-commerce/src/services/marketplace_provider_reversal_event_adapter.rs`
- `crates/rustok-commerce/src/services/marketplace_reversal_event_inbox.rs`
- `crates/rustok-commerce/src/services/marketplace_reversal_operator.rs`
- `crates/rustok-commerce/src/services/marketplace_financial_operator.rs`
- `crates/rustok-commerce/src/services/marketplace_financial_runtime.rs`
- `crates/rustok-commerce/src/controllers/marketplace_financial.rs`
- `crates/rustok-commerce/src/controllers/marketplace_reversal_financial.rs`
- `crates/rustok-commerce/src/graphql/marketplace_financial.rs`
- `crates/rustok-commerce/contracts/marketplace-reversal-recovery-v1.json`
- `crates/rustok-commerce/tests/marketplace_reversal_recovery_source.rs`
- `apps/server/src/services/payment_provider_event_worker.rs`
- `apps/server/src/services/marketplace_financial_worker.rs`
- `apps/server/src/services/commerce_provider_runtime.rs`
- `apps/server/src/services/module_event_dispatcher.rs`
- `crates/rustok-commerce/src/services/checkout_stage_pipeline.rs`
- `crates/rustok-moderation/`
