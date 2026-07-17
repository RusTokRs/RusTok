# RusToK ecommerce implementation plan

Last reviewed: 2026-07-17

## Source of truth

This file is the only human-maintained source of truth for ecommerce
implementation tasks, completion marks, verification state, execution order, and
promotion gates.

Rules:

- `[x]` means source or retained execution evidence exists in `main`.
- `[ ]` means implementation or required evidence is still missing.
- Source implementation and runtime verification are separate tasks.
- Owner runbooks, local plans, and contracts describe behavior and evidence; this
  file controls the cross-ecommerce execution order and promotion state.
- Newly discovered work is recorded here before or with its implementation.
- No FBA or FFA status is promoted from source inspection alone.

`rustok-commerce` owns general ecommerce cross-domain orchestration. Product, cart,
customer, region, pricing, inventory, order, payment, fulfillment, tax, promotion,
and market/store remain owner bounded contexts. Marketplace capabilities belong to
the explicit `rustok-marketplace-*` family; seller, listing, allocation,
commission, ledger, and payout persistence must never be folded into
`rustok-commerce`.

## Current boundary

- Ecommerce FFA status: `in_progress`.
- Ecommerce FBA status: `boundary_ready`.
- Ecommerce structural shape: `core_transport_ui`.
- Payment FFA status: `in_progress`.
- Payment FBA status: `boundary_ready`.
- Marketplace family source gate: `open`.
- Marketplace root FFA status: `not_started`.
- Marketplace root FBA status: `in_progress`.
- Marketplace root structural shape: `no_ui_boundary`.
- Marketplace seller FFA status: `in_progress`.
- Marketplace seller FBA status: `in_progress`.
- Marketplace seller structural shape: `core_transport_ui`.
- Marketplace listing FFA status: `not_started`.
- Marketplace listing FBA status: `in_progress`.
- Marketplace listing structural shape: `no_ui_boundary`.
- Marketplace production promotion gate: `closed` until compile, migration,
  contention, restart, mounted transport, remote-profile, and complete
  owner/provider-consumer evidence is retained.

Registries and evidence:

- `crates/rustok-commerce/contracts/commerce-fba-registry.json`
- `crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json`
- `crates/rustok-pricing/contracts/pricing-fba-registry.json`
- `crates/rustok-inventory/contracts/inventory-fba-registry.json`
- `crates/rustok-order/contracts/order-fba-registry.json`
- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`
- `crates/rustok-product/contracts/product-fba-registry.json`
- `crates/rustok-customer/contracts/customer-fba-registry.json`
- `crates/rustok-cart/contracts/cart-fba-registry.json`
- [x] `crates/rustok-marketplace/contracts/marketplace-fba-registry.json`
- [x] `crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json`
- [x] `crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json`
- [x] `apps/server/tests/marketplace_family_boundary_guard.rs`
- [x] `apps/server/tests/marketplace_seller_transport_guard.rs`
- [x] `apps/server/tests/marketplace_listing_boundary_guard.rs`
- [x] `apps/server/tests/marketplace_listing_lifecycle_event_guard.rs`
- [x] `scripts/verify/verify-marketplace-family-boundary.mjs`
- [x] `scripts/verify/verify-marketplace-seller-transport.mjs`
- [x] `scripts/verify/verify-marketplace-listing-boundary.mjs`
- [x] `scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`

## Architecture invariants

### General ecommerce

- [x] Keep checkout and cross-domain recovery in `rustok-commerce`.
- [x] Keep owner persistence and lifecycle transitions in owner modules.
- [x] Use typed ports or explicit owner runtime APIs instead of foreign entities.
- [x] Carry tenant, actor, effective locale, channel, correlation, deadline, and
  idempotency context across owner calls.
- [x] Keep payment/refund lifecycle persistence in `rustok-payment`.
- [x] Keep provider payload parsing and signature verification outside commerce.
- [x] Never persist or expose raw provider payloads, signatures, SQL messages, SDK
  errors, or KYC provider payloads.

### FBA/FFA modules

- [x] Every owner module keeps domain policy, persistence, receipts/events, and
  lifecycle commands inside the owner crate.
- [x] FBA exposes typed ports using `PortContext`, stable `PortError`, deadline rules,
  retryability, and command idempotency admission.
- [x] In-process providers implement the same typed contracts expected by future
  remote adapters.
- [x] FFA packages own framework-neutral core/model, transport facade, i18n, and a
  thin UI adapter; hosts only compose them.
- [x] Native/GraphQL transport selection is explicit; automatic fallback is
  forbidden unless a contract explicitly defines it and retained evidence exists.
- [x] Use the structural sequence `core_only -> core_transport ->
  core_transport_ui`.
- [ ] Retain compiled in-process/remote-profile evidence before FBA
  `transport_verified`.
- [ ] Retain mounted native/GraphQL parity before FFA `phase_b_ready`.

## Checkout and post-order orchestration

### Admission and durable stages

- [x] Require and reuse stable checkout idempotency at REST, GraphQL, native, and UI
  boundaries.
- [x] Route production checkout through `StagedCheckoutService`.
- [x] Read/mutate cart and resolve product, pricing, inventory, order, payment, and
  fulfillment through owner boundaries.
- [x] Validate channel, locale, region, shipping, product, price, and inventory before
  external effects.
- [x] Persist immutable order/fulfillment plans, operation identity, request/cart
  hashes, lease, stage, errors, and owner ids.
- [x] Resume all persisted checkout stages and adopt already committed owner results.
- [x] Prevent a second active checkout for the same cart.
- [x] Route REST, GraphQL, native, and compatibility wrappers through one recovering
  staged runtime.
- [ ] Retain native/REST/GraphQL parity evidence for admission failures.
- [ ] Add and execute kill points after every owner call and before every checkpoint.
- [ ] Prove restart does not duplicate reservations, orders, collections, provider
  operations, fulfillments, labels, or cart completion.

### Compensation and reconciliation

- [x] Separate pre-order reservation release from post-order cancellation.
- [x] Avoid automatic reversal of captured or uncertain provider effects.
- [x] Provide synchronous safe compensation and a lease-protected sweep.
- [x] Classify manual financial work as `reconciliation_required` and block new
  provider execution while reconciliation is open.
- [x] Publish safe admin reads, compensation commands, and bounded sweep routes.
- [ ] Publish explicit operator reconciliation-resolution commands; automatic retry
  remains forbidden for `reconciliation_required`.
- [ ] Prove compensation contention and restart behavior on PostgreSQL.
- [ ] Execute complete mounted operator workflows.

### Return completion

- [x] Keep refund, exchange, and claim return-completion coordination in
  `ReturnCompletionOrchestrationService`.
- [x] Persist `return_completion_operations` with canonical request hash, typed
  stages, lease/CAS execution, owner resolution identities, safe errors, and
  terminal timestamps.
- [x] Persist immutable return-completion command snapshots with original actor and
  retry audit.
- [x] Atomically admit command plus pending operation before provider/owner effects.
- [x] Adopt existing refunds, order changes, and completed returns; reject conflicting
  replay payloads.
- [x] Classify uncertain provider outcomes as `reconciliation_required`.
- [x] Publish tenant-scoped operator list/show/retry routes without exposing stored
  command payloads.
- [ ] Apply return-completion migrations on clean/upgraded SQLite/PostgreSQL,
  including rollback/reapply.
- [ ] Execute duplicate replay, conflicting payload, concurrent admission/claim,
  expired lease, process exit, and restart recovery scenarios.

Checkout/post-order evidence:

- `crates/rustok-commerce/src/services/staged_checkout.rs`
- `crates/rustok-commerce/src/services/checkout_stage_pipeline.rs`
- `crates/rustok-commerce/src/services/checkout_compensation.rs`
- `crates/rustok-commerce/src/services/recovering_staged_checkout.rs`
- `crates/rustok-commerce/src/services/fulfillment_orchestration_facade.rs`
- `crates/rustok-commerce/src/services/order_change_orchestration.rs`
- `crates/rustok-commerce/src/entities/return_completion_operation.rs`
- `crates/rustok-commerce/src/entities/return_completion_command.rs`
- `crates/rustok-commerce/src/services/return_completion_operation.rs`
- `crates/rustok-commerce/src/services/return_completion_orchestration.rs`
- `crates/rustok-commerce/src/services/return_completion_recovery.rs`
- `crates/rustok-commerce/src/controllers/return_completion_operations.rs`
- `apps/server/tests/commerce_return_completion_transport_guard.rs`

## Marketplace Family

Marketplace owner-domain source work proceeds in parallel with ecommerce runtime
evidence. It must not weaken checkout/payment/return boundaries and must not be
promoted as production-ready before the family promotion gates close.

### Naming and ownership completed

- [x] Create `rustok-marketplace` as the family root and composition/orchestration
  boundary; it owns no seller, listing, allocation, commission, ledger, or payout
  tables.
- [x] Create `rustok-marketplace-seller` as the seller owner.
- [x] Create `rustok-marketplace-listing` as the listing owner.
- [x] Require the `rustok-marketplace-*` crate family and `marketplace_*` module slugs.
- [x] Forbid generic marketplace owner names such as `rustok-seller`,
  `rustok-offer`, `rustok-listing`, `rustok-commission`, `rustok-ledger`, and
  `rustok-payout`.
- [x] Keep master catalog in `rustok-product`, price/rules in `rustok-pricing`, stock
  in `rustok-inventory`, customer order lifecycle in `rustok-order`, provider
  payment/refund state in `rustok-payment`, and cross-domain orchestration in
  `rustok-commerce`.
- [x] Register seller, listing, and root Marketplace features as opt-in; do not
  default-enable them before runtime evidence exists.
- [ ] Create future owner modules using mandatory family names:
  `rustok-marketplace-commission`, `rustok-marketplace-ledger`, and
  `rustok-marketplace-payout`.

### Marketplace root FBA/FFA

- [x] Publish typed root consumers for seller directory, listing directory, and
  listing eligibility.
- [x] Keep root consumers free of owner entities, SeaORM, and owner database access.
- [x] Publish family/provider registries and source guards.
- [ ] Add allocation, commission, ledger, and payout consumers only after their owner
  contracts exist.
- [ ] Compile and execute root seller/listing consumers.
- [ ] Retain remote-profile timeout/degraded/fallback evidence.
- [ ] Keep root FFA `not_started`; future control-room UI may only compose owner view
  models and transport facades.

### Seller owner completed

- [x] Own seller identity, legal profile, onboarding/lifecycle, memberships, roles,
  and seller-scoped policy.
- [x] Keep platform RBAC separate from seller membership policy.
- [x] Keep `marketplace_sellers` language-agnostic and localized `display_name` in
  `marketplace_seller_translations`.
- [x] Enforce `(tenant_id, seller_id, locale)` translation identity, normalized
  locale tags, and `VARCHAR(32)` storage.
- [x] Use exact effective locale from `PortContext`; seller storage does not invent a
  fallback chain.
- [x] Return `resolved_locale` through FBA and FFA projections.
- [x] Create seller, translation, and initial owner membership atomically.
- [x] Persist durable command receipts for every FBA seller write with actor-bound
  canonical SHA-256 identity, normalized result snapshot, and conflicting-payload
  rejection.
- [x] Use SQL `ON CONFLICT` for translation upsert.
- [x] Route every seller FBA command through the durable receipt executor; direct
  non-receipted writes are not an FBA path.
- [x] Publish module-owned seller FFA native/GraphQL source workflows with explicit
  transport selection, retry key preservation, and manifest-driven host
  composition.

### Seller owner remaining

- [ ] Replace mutable onboarding/suspension prose with immutable locale-tagged seller
  lifecycle/moderation events.
- [ ] Add bounded event timeline reads to FBA and seller FFA workflows.
- [ ] Backfill/remove mutable compatibility snapshots after complete event coverage.
- [ ] Publish seller events through the transactional outbox.
- [ ] Add normalized verification facts and KYC provider SPI without raw payload
  persistence.
- [ ] Compile seller/core/GraphQL/admin packages, apply migrations, and execute
  replay, locale, tenant, contention, rollback, mounted native/GraphQL, and
  remote-profile evidence.

Seller evidence:

- `crates/rustok-marketplace-seller/src/entities/seller.rs`
- `crates/rustok-marketplace-seller/src/entities/seller_translation.rs`
- `crates/rustok-marketplace-seller/src/entities/seller_command_receipt.rs`
- `crates/rustok-marketplace-seller/src/localized_sellers.rs`
- `crates/rustok-marketplace-seller/src/command_receipts.rs`
- `crates/rustok-marketplace-seller/src/receipted_commands.rs`
- `crates/rustok-marketplace-seller/src/ports.rs`
- `crates/rustok-marketplace-seller/src/graphql.rs`
- `crates/rustok-marketplace-seller/admin/src/transport/native_server_adapter.rs`
- `crates/rustok-marketplace-seller/admin/src/transport/graphql_adapter.rs`
- `crates/rustok-marketplace-seller/admin/src/ui/leptos.rs`

### Listing owner completed

- [x] Own seller/master-variant/market/channel listing identity, seller SKU,
  lifecycle, approval, versioned commercial terms, and deterministic eligibility.
- [x] Keep canonical product localized copy, prices, stock, and fulfillment state in
  their owner modules.
- [x] Store no localized product title/description or locale-keyed JSON maps in
  listing tables.
- [x] Use no cross-module database foreign keys.
- [x] Publish `MarketplaceListingReadPort` and `MarketplaceListingCommandPort` with
  deadline/idempotency/error rules.
- [x] Resolve seller facts through `MarketplaceSellerReadPort` and product facts
  through `ProductCatalogReadPort`.
- [x] Persist durable listing command receipts and normalized result snapshots.
- [x] Route create, publish, and reactivate through replay admission before external
  provider reads.
- [x] Add append-only `marketplace_listing_events` with tenant/listing scope, actor,
  typed event kind, normalized effective locale, note, metadata, timestamp, and
  bounded newest-first timeline reads.
- [x] Route terms update, submit-for-review, approval/rejection, suspension, and
  archive through atomic owner state/terms + event + receipt transactions.
- [x] Include effective locale in event-producing command request identity.
- [x] Register listing in modules, distribution, and server as opt-in backend owner
  composition.

### Listing owner remaining

- [ ] Add immutable `created`, `published`, and `reactivated` events while preserving
  replay admission before seller/product reads.
- [ ] Remove remaining direct compatibility lifecycle paths after complete event
  coverage.
- [ ] Backfill events from `approval_note` and `suspension_reason`, then drop mutable
  compatibility columns.
- [ ] Publish listing events through the transactional outbox.
- [ ] Add product matching/approval workflow before automated EAN/GTIN matching,
  deduplication, or buy-box ranking.
- [ ] Create `rustok-marketplace-listing-admin` using the FFA
  core/model/transport/i18n/Leptos structure with explicit native/GraphQL selection.
- [ ] Compile listing/root/provider contracts, apply SQLite/PostgreSQL migrations,
  and execute replay, locale, tenant, event atomicity, contention, rollback, restart,
  and mounted transport evidence.

Listing evidence:

- `crates/rustok-marketplace-listing/src/entities/listing.rs`
- `crates/rustok-marketplace-listing/src/entities/listing_terms.rs`
- `crates/rustok-marketplace-listing/src/entities/listing_event.rs`
- `crates/rustok-marketplace-listing/src/entities/listing_command_receipt.rs`
- `crates/rustok-marketplace-listing/src/command_receipts.rs`
- `crates/rustok-marketplace-listing/src/replay_safe_commands.rs`
- `crates/rustok-marketplace-listing/src/listing_events.rs`
- `crates/rustok-marketplace-listing/src/evented_commands.rs`
- `crates/rustok-marketplace-listing/src/lifecycle_event_commands.rs`
- `crates/rustok-marketplace-listing/src/ports.rs`
- `crates/rustok-marketplace-listing/contracts/marketplace-listing-fba-registry.json`

### Marketplace order allocation

- [ ] Introduce durable seller order groups/allocations without duplicating the
  customer order aggregate.
- [ ] Snapshot seller, listing, commission policy, fulfillment ownership, and
  monetary allocation on order lines at checkout.
- [ ] Route seller-specific fulfillment, cancellation, return, claim, and refund
  decisions through commerce/marketplace orchestration and owner commands.
- [ ] Prevent one seller lifecycle operation from mutating another seller allocation
  or financial state.

### Commission, ledger, and payout

- [ ] Create `rustok-marketplace-commission` with versioned policies and
  deterministic calculation explanations; snapshot applied policy/results on order
  allocations.
- [ ] Create immutable double-entry `rustok-marketplace-ledger` before balances or
  payouts.
- [ ] Derive pending, available, reserved, disputed, and paid balances only from
  ledger entries.
- [ ] Create `rustok-marketplace-payout` with idempotent payout journals, provider
  SPI, retries, reconciliation, reversals, and operator audit.
- [ ] Keep split-payment provider capabilities optional; internal allocation and
  ledger correctness must not depend on a specific PSP.

### Marketplace FBA/FFA promotion gates

- [ ] FBA `boundary_ready`: compiled typed provider/consumer ports, manifests,
  in-process providers, registries, durable command identity, locale/event rules,
  migrations, source guards, and contract cases are retained.
- [ ] FBA `transport_verified`: in-process and remote-profile timeout, degraded,
  fallback, and mounted consumer evidence are retained.
- [ ] FFA `phase_b_ready`: module-owned core/model/transport/i18n/UI package, host
  composition, and native/GraphQL parity evidence are retained.
- [ ] FFA `parity_verified`: supported hosts render equivalent workflows and errors
  without importing owner internals.
- [ ] Marketplace production-ready: clean/upgraded migrations, tenant isolation,
  contention/restart recovery, mounted admin/vendor/storefront flows, allocation,
  ledger, and payout reconciliation evidence are retained.

## Payment workstream

### Ownership and provider SPI

- [x] Keep collections, payments, refunds, lifecycle state, provider-operation
  journals, and webhook inbox state in `rustok-payment`.
- [x] Publish typed `PaymentCollectionPort` with write idempotency.
- [x] Keep native storefront transport host-neutral and retain GraphQL compatibility
  reads.
- [x] Publish provider descriptors, capabilities, health, degraded mode, and
  registration validation.
- [x] Guard authorize, capture, cancel, refund, and webhook operations through
  `PaymentProviderRegistry`.
- [x] Persist provider-operation requests/results with CAS execution and explicit
  reconciliation outcomes.
- [x] Route uncertain external outcomes to `reconciliation_required` and forbid
  automatic re-claim.
- [x] Add tenant-scoped Stripe provider source and deployment-owned secret resolution.
- [ ] Execute deployment secret resolution and authorize/capture/cancel/refund/webhook
  handling against a production-like Stripe endpoint.
- [ ] Prove adapters never persist payment/refund owner lifecycle state.

### Refund identity

- [x] Add refund `creation_key` and canonical `creation_request_hash` with
  tenant/collection uniqueness and immutable guards.
- [x] Route direct REST/GraphQL refund creation through the idempotent owner service.
- [x] Remove identity-less refund creation APIs.
- [x] Add replay, conflicting payload, same-key contention, and database hard-stop
  source/tests.
- [ ] Retain mounted REST/GraphQL/native parity and provider evidence.

### Webhook ingress and recovery

- [x] Mount provider webhook ingress with tenant scope, signature validation, body
  limit, authoritative delivery/replay identity, and immutable normalized facts.
- [x] Persist only digest plus verified normalized facts; raw provider bodies are not
  stored.
- [x] Apply owner events before marking inbox processed.
- [x] Recover received/failed/expired-processing events and isolate failures.
- [x] Exclude dead-letter rows from automatic retry and publish operator-only replay.
- [x] Run bounded recovery in the shared server worker lifecycle.
- [ ] Execute real Stripe signature verification over mounted HTTP ingress.
- [ ] Retain malformed-signature, duplicate, unsupported, out-of-order,
  hint-conflict, restart, expired-lease, replica, and operator replay evidence.

Payment evidence:

- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-payment/src/providers.rs`
- `crates/rustok-payment/src/stripe_provider.rs`
- `crates/rustok-payment/src/services/refund_creation.rs`
- `apps/server/src/services/payment_provider_runtime.rs`
- `apps/server/src/services/payment_provider_event_worker.rs`
- `scripts/verify/verify-payment-refund-identity.mjs`
- `scripts/verify/verify-payment-provider-outcome-contract.mjs`
- `scripts/verify/verify-payment-stripe-runtime.mjs`

## Cross-domain evidence

- [x] Execute one compiled product/cart/inventory checkout validation proving
  channel-hidden inventory blocks checkout.
- [ ] Execute all product, cart, customer, region, pricing, inventory, order,
  payment, fulfillment, and marketplace ports with real deadlines.
- [ ] Retain retryable, unavailable, degraded, timeout, malformed, and fallback
  evidence for every provider/consumer pair.
- [ ] Prove equivalent native, REST, GraphQL, and module-owned FFA behavior.
- [ ] Replace placeholder/static packets with observed evidence.

## Verification and promotion checklist

Source inspection is not execution evidence. The following remain unchecked until
explicitly executed and retained.

### Static verification

- [ ] `cargo fmt --all -- --check`
- [ ] `npm run verify:ecommerce:fba`
- [ ] `npm run verify:ecommerce:provider-spi-evidence`
- [ ] `npm run verify:commerce:admin-boundary`
- [ ] `npm run verify:commerce:storefront-transport-handoff`
- [ ] `npm run verify:payment:storefront-boundary`
- [ ] `npm run verify:payment:refund-identity`
- [ ] `npm run verify:payment:provider-outcomes`
- [ ] `npm run verify:payment:stripe-runtime`
- [ ] `node scripts/verify/verify-marketplace-family-boundary.mjs`
- [ ] `node scripts/verify/verify-marketplace-seller-transport.mjs`
- [ ] `node scripts/verify/verify-marketplace-listing-boundary.mjs`
- [ ] `node scripts/verify/verify-marketplace-listing-lifecycle-events.mjs`
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`
- [ ] `cargo xtask module validate marketplace`
- [ ] `cargo xtask module validate marketplace_seller`
- [ ] `cargo xtask module validate marketplace_listing`

### Compile and tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-marketplace --lib`
- [ ] `cargo check -p rustok-marketplace-seller --lib`
- [ ] `cargo check -p rustok-marketplace-seller-admin --all-features`
- [ ] `cargo check -p rustok-marketplace-listing --lib`
- [ ] `cargo check -p rustok-server --features payment-stripe,mod-commerce`
- [ ] `cargo check -p rustok-server --features mod-marketplace`
- [ ] Targeted checkout, return-completion, refund identity, provider-operation,
  provider-event, seller/listing lifecycle, localization, membership, event timeline,
  port contract, replay, recovery, and tenant-isolation tests.

### Database and runtime

- [ ] Apply clean and upgraded SQLite/PostgreSQL graphs and supported rollback/reapply
  paths.
- [ ] Execute checkout, return-completion, refund, provider-operation, provider-event,
  seller, listing, translation, receipt, lifecycle-event, and timeline contention.
- [ ] Execute marketplace seller/listing tenant isolation and cross-locale scenarios.
- [ ] Prove all declared routers and module-owned UI packages are mounted.
- [ ] Exercise authenticated checkout, return-completion recovery, compensation,
  seller administration, listing administration, reconciliation, recovery, and
  replay.
- [ ] Prove workers obey runtime profile/shutdown and replicas cannot double-apply.
- [ ] Retain real payment signature, redelivery, degraded, reconciliation, and
  operator evidence.

## Immediate execution order

The capability and evidence tracks proceed in parallel. Marketplace source work
must not wait for every external-adapter proof, and evidence work must not be
abandoned while marketplace capabilities are added.

1. [x] Add immutable return-completion command snapshots, atomic command/operation
   admission, and operator list/show/retry routes.
2. [x] Create Marketplace root and seller owner with typed FBA boundaries.
3. [x] Create seller FFA package and explicit native/GraphQL source workflows.
4. [x] Add durable seller receipts, exact-locale translation storage, and transport
   locale parity.
5. [x] Create listing owner with versioned terms, eligibility, receipts, and opt-in
   module composition.
6. [x] Add locale-tagged immutable listing event storage and route terms update,
   submit, review, suspend, and archive through atomic evented commands.
7. [ ] Add immutable listing `created`, `published`, and `reactivated` events while
   preserving provider-preflight replay.
8. [ ] Backfill listing events and remove mutable note compatibility columns.
9. [ ] Add immutable seller lifecycle/moderation events and timeline reads.
10. [ ] Create listing FFA package with explicit native/GraphQL transports.
11. [ ] Run static ecommerce/payment/marketplace verifiers and fix drift.
12. [ ] Run commerce, payment, Marketplace, Stripe-feature, and server compile checks.
13. [ ] Run clean/upgraded SQLite/PostgreSQL migrations and targeted regression tests.
14. [ ] Run contention, restart, and kill-point scenarios for checkout, return
    completion, payment operations, webhook recovery, seller, and listing writes.
15. [ ] Introduce seller order allocations and commission snapshots after seller and
    listing owner contracts are source-complete.
16. [ ] Build the double-entry marketplace ledger before payout commands or balances.
17. [ ] Add payout journals/provider SPI only after ledger invariants are retained.
18. [ ] Execute deployment secret resolution and Stripe against a production-like
    endpoint.
19. [ ] Run mounted HTTP, native/GraphQL FFA, and background-worker recovery scenarios.
20. [ ] Reassess FBA/FFA promotion strictly from retained evidence.

## Change rules

1. Update this file with every completed or newly discovered ecommerce task.
2. Keep local owner plans synchronized with this file; they may contain owner detail
   but must not contradict central status or execution order.
3. Owner modules retain domain invariants, persistence, provider policy, receipts,
   events, and owner commands.
4. Host applications and family roots compose typed ports and FFA packages; they do
   not own domain policy.
5. Never persist or expose raw provider payloads, signatures, SQL messages, SDK
   errors, or KYC provider payloads.
6. Keep manifests, registries, source guards, OpenAPI/GraphQL contracts, runbooks,
   local plans, and this plan aligned.
7. Update `docs/modules/registry.md` whenever a local FFA/FBA status changes.
8. Marketplace capability crates and module slugs must preserve the
   `rustok-marketplace-*` / `marketplace_*` family identity.

## References

- [Commerce crate README](../README.md)
- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Payment webhook runbook](../../rustok-payment/docs/provider-webhooks.md)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
- [Marketplace root plan](../../rustok-marketplace/docs/implementation-plan.md)
- [Marketplace seller plan](../../rustok-marketplace-seller/docs/implementation-plan.md)
- [Marketplace listing plan](../../rustok-marketplace-listing/docs/implementation-plan.md)
