# RusToK ecommerce implementation plan

Last reviewed: 2026-07-16

## Source of truth

This file is the only human-maintained source of truth for ecommerce
implementation tasks, completion marks, verification state, execution order, and
promotion gates.

Rules:

- `[x]` means source or retained execution evidence exists in `main`.
- `[ ]` means implementation or required evidence is still missing.
- Source implementation and runtime verification are separate tasks.
- Owner runbooks and contracts describe behavior, not a second roadmap.
- Newly discovered work is recorded here before or with its implementation.

`rustok-commerce` owns cross-domain orchestration. Product, cart, customer, region,
pricing, inventory, order, payment, fulfillment, tax, promotion, market/store,
seller/offer, commission, ledger, and payout remain owner bounded contexts.

## Current boundary

- FFA status: `in_progress`.
- FBA status: `boundary_ready`.
- Structural shape: `core_transport_ui`.
- Payment FFA status: `in_progress`.
- Payment FBA status: `boundary_ready`.
- Marketplace foundation source gate: `open`.
- Marketplace production promotion gate: `closed` until compile, migration,
  contention, restart, and mounted transport evidence is retained.
- Source-only work never promotes a boundary without compile, migration,
  transport, concurrency, and external-adapter evidence.

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

## Architecture invariants

- [x] Keep checkout and cross-domain recovery in `rustok-commerce`.
- [x] Keep owner persistence and lifecycle transitions in owner modules.
- [x] Use typed ports or explicit owner runtime APIs instead of foreign entities.
- [x] Carry tenant, actor, locale, channel, correlation, deadline, and idempotency
  context across owner calls.
- [x] Keep payment/refund lifecycle persistence in `rustok-payment`.
- [x] Keep provider payload parsing and signature verification outside commerce.
- [x] Keep this file as the only ecommerce checklist.
- [x] Enforce the payment planning redirect through source guardrails.
- [x] Route GraphQL fulfillment create, ship, deliver, reopen, reship, and cancel
  through `FulfillmentOrchestrationService` instead of direct owner-service calls.
- [x] Guard fulfillment transport ownership with
  `apps/server/tests/commerce_fulfillment_transport_guard.rs` and
  `verify-commerce-admin-boundary.mjs`.
- [x] Keep order-change type dispatch and exchange/claim refund coordination in
  `OrderChangeOrchestrationService` instead of transport code.
- [x] Route REST and GraphQL order-change application through the same
  `OrderChangeOrchestrationService`.
- [x] Guard REST and GraphQL order-change orchestration ownership with
  `apps/server/tests/commerce_order_change_transport_guard.rs` and
  `verify-commerce-admin-boundary.mjs`.
- [x] Keep refund, exchange, and claim return-completion coordination in
  `ReturnCompletionOrchestrationService` instead of REST or GraphQL transports.
- [x] Validate the complete return command before provider or owner side effects,
  including mutually exclusive helpers and explicit resolution references.
- [x] Remove the legacy GraphQL provider-refund helper module and guard the shared
  return-completion boundary with
  `apps/server/tests/commerce_return_completion_transport_guard.rs` and
  `verify-commerce-admin-boundary.mjs`.
- [x] Persist one `return_completion_operations` row per tenant/return with an
  immutable canonical SHA-256 request hash, typed stages, lease/CAS execution,
  owner resolution identities, safe errors, and terminal timestamps.
- [x] Admit the journal before provider or owner effects, adopt existing refunds,
  order changes, and completed returns, and reject conflicting replay payloads.
- [x] Bind generated exchange/claim changes to
  `return_completion_operation_id`, validate explicit refund/change references
  against the return order, and classify uncertain provider outcomes as
  `reconciliation_required`.
- [x] Persist one immutable canonical return-completion command snapshot with the
  original actor and retry audit, and atomically create its pending operation in
  the same database transaction before execution.
- [x] Route REST and GraphQL through the durable recovery facade while keeping
  provider/owner effects in the core return-completion orchestration.
- [x] Publish tenant-scoped operator list/show/retry routes without exposing the
  stored command payload; require `orders:manage` plus `payments:manage` for retry.
- [x] Guard return-completion schema, command admission, replay, lease, adoption,
  operator retry, payload secrecy, and reconciliation source invariants in
  `commerce_return_completion_transport_guard.rs`.
- [ ] Apply migrations `m20260716_000004_create_return_completion_operations`
  through `m20260716_000006_create_return_completion_commands` on clean and
  upgraded SQLite/PostgreSQL graphs, including rollback/reapply.
- [ ] Execute duplicate replay, conflicting payload, concurrent admission/claim,
  expired lease, process exit, and restart recovery after refund, order-change,
  and owner return-completion checkpoints.
- [ ] Publish explicit operator reconciliation-resolution commands; automatic
  retry must remain forbidden for `reconciliation_required`.
- [ ] Execute the complete provider-consumer graph with retained runtime evidence.

## Checkout orchestration workstream

### Admission and immutable plan

- [x] Require and reuse a stable checkout idempotency key at REST, GraphQL, native,
  and UI boundaries.
- [x] Route production checkout through `StagedCheckoutService`.
- [x] Read and mutate cart state through cart-owned ports.
- [x] Resolve product and pricing through owner projections.
- [x] Validate channel, locale, region, shipping, product, price, and inventory
  before external effects.
- [x] Persist one immutable order/fulfillment plan.
- [ ] Retain native/REST/GraphQL parity evidence for admission failures.

### Durable stages and replay

- [x] Persist operation identity, request/cart hashes, lease, stage, errors, and
  owner ids.
- [x] Resume `cart_locked`, `inventory_reserved`, `order_created`,
  `payment_ready`, `payment_authorized`, `payment_captured`,
  `fulfillment_created`, `cart_completed`, and `completed`.
- [x] Adopt inventory reservation identities and prevent legacy double reserve.
- [x] Accept already committed order, payment, fulfillment, and cart results.
- [x] Prevent a second active checkout for the same cart.
- [x] Route REST, GraphQL, native, and the compatibility journal wrapper through
  one recovering staged runtime.
- [ ] Add and execute kill points after every owner call and before every
  checkpoint.
- [ ] Prove restart does not duplicate reservations, orders, collections, provider
  operations, fulfillments, labels, or cart completion.

### Compensation and reconciliation

- [x] Separate pre-order reservation release from post-order cancellation.
- [x] Release adopted inventory through order ownership.
- [x] Avoid automatic reversal of captured or uncertain provider effects.
- [x] Provide synchronous safe compensation and a lease-protected sweep.
- [x] Classify manual financial work as `reconciliation_required`.
- [x] Block new checkout/provider execution while reconciliation is open.
- [x] Publish safe admin reads, compensation commands, and bounded sweep routes.
- [ ] Prove compensation contention and restart behavior on PostgreSQL.
- [ ] Execute complete mounted operator workflows.

Checkout and post-order evidence:

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
- `crates/rustok-commerce/src/migrations/m20260716_000004_create_return_completion_operations.rs`
- `crates/rustok-commerce/src/migrations/m20260716_000005_enforce_return_completion_resolution_identity.rs`
- `crates/rustok-commerce/src/migrations/m20260716_000006_create_return_completion_commands.rs`
- `crates/rustok-commerce/src/controllers/return_completion_operations.rs`
- `crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs`
- `apps/server/tests/commerce_fulfillment_transport_guard.rs`
- `apps/server/tests/commerce_order_change_transport_guard.rs`
- `apps/server/tests/commerce_return_completion_transport_guard.rs`
- `scripts/verify/verify-commerce-admin-boundary.mjs`
- `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`

## Marketplace foundation gate

Marketplace owner-domain implementation may start now. It does not wait for all
production evidence above, but it must not weaken checkout/payment/return
boundaries or be promoted as production-ready before those evidence gates close.

### Seller and membership

- [ ] Create `rustok-seller` as the owner of seller identity, legal/display profile,
  onboarding state, suspension state, and seller lifecycle transitions.
- [ ] Add seller memberships with immutable seller scope, roles, invitation state,
  and tenant isolation; integrate permissions without encoding seller policy in
  `rustok-commerce`.
- [ ] Publish seller admin/portal read and command ports with deadline,
  authorization, and idempotency contracts.
- [ ] Add seller onboarding review/audit events; keep KYC provider details behind a
  provider SPI and store only normalized verification facts.

### Master catalog and offers

- [ ] Create `rustok-offer` as the owner of seller offers linked to product-owned
  master products/variants; do not fork canonical product content into seller
  tables.
- [ ] Model offer status, seller SKU, price reference, inventory reference,
  fulfillment profile, market/channel visibility, publication, and approval.
- [ ] Enforce one active seller offer identity per seller/master variant/market
  scope while allowing versioned commercial terms.
- [ ] Publish deterministic offer eligibility and selection projections for cart,
  pricing, inventory, search, and storefront consumers.
- [ ] Add product matching/approval workflows before automatic EAN/GTIN matching,
  deduplication, or buy-box ranking.

### Marketplace order ownership

- [ ] Introduce durable order groups and seller allocations without duplicating the
  customer order aggregate.
- [ ] Snapshot seller, offer, commission policy, fulfillment ownership, and monetary
  allocation on order lines at checkout.
- [ ] Route seller-specific fulfillment, cancellation, return, claim, and refund
  decisions through commerce orchestration and owner commands.
- [ ] Prevent one seller's lifecycle operation from mutating another seller's
  allocation or financial state.

### Commission, ledger, and payout

- [ ] Create `rustok-commission` with versioned policies and deterministic
  calculation explanations; snapshot the applied policy/result on order
  allocations.
- [ ] Create an immutable double-entry `rustok-ledger` before implementing balances
  or payouts.
- [ ] Derive pending, available, reserved, disputed, and paid seller balances only
  from ledger entries.
- [ ] Create `rustok-payout` with idempotent payout journals, provider SPI,
  retries, reconciliation, reversals, and operator audit.
- [ ] Keep split-payment provider capabilities optional; internal allocation and
  ledger correctness must not depend on a specific PSP.

### Marketplace surfaces and advanced capabilities

- [ ] Build vendor portal and platform-admin transports over seller/offer/order/
  ledger/payout owner APIs; UI must not own marketplace policy.
- [ ] Add storefront multi-seller offer display and deterministic selection before
  implementing buy-box ranking.
- [ ] Add multi-channel stock sync, KYC adapters, automated catalog matching, and
  PSP split payouts only after the corresponding owner contracts and recovery
  journals are proven.

## Payment workstream

`crates/rustok-payment/docs/implementation-plan.md` is a compatibility redirect to
this section.

### Ownership and checkout boundary

- [x] Keep collections, payments, refunds, and lifecycle state in
  `rustok-payment`.
- [x] Keep `PaymentService` as the lifecycle owner after provider operations and
  webhook normalization.
- [x] Publish typed `PaymentCollectionPort` with write idempotency.
- [x] Keep native storefront transport host-neutral through `HostRuntimeContext`.
- [x] Retain GraphQL fallback for payment collection and refund summary reads.
- [x] Lock storefront ownership with
  `verify-payment-storefront-boundary.mjs`.
- [ ] Execute the checkout payment port through a real remote adapter.
- [ ] Retain timeout, typed-error, fallback, cart-ownership, and transport-parity
  evidence.

### Provider SPI and outbound operations

- [x] Publish descriptors, capabilities, health, degraded mode, and registration
  validation.
- [x] Keep the manual provider as a baseline adapter.
- [x] Guard authorize, capture, cancel, refund, and webhook operations through
  `PaymentProviderRegistry`.
- [x] Persist provider-operation requests/results with CAS execution and explicit
  reconciliation outcomes.
- [x] Recover external payment identity for capture, cancel, and refund from the
  durable authorize journal.
- [x] Route refund provider execution through the common journal executor.
- [x] Add refund `creation_key` and canonical `creation_request_hash` with
  tenant/collection uniqueness and immutable database guards.
- [x] Add `PaymentRefundCreationService::create_or_replay` with captured-state,
  refundable-capacity, replay, request-conflict, and insert-race handling.
- [x] Require REST `Idempotency-Key` and GraphQL `idempotencyKey` for direct refund
  creation.
- [x] Use deterministic return/change identities for post-order refund workflows.
- [x] Require creation identity for every new refund row and backfill legacy rows
  through migrations `000118` and `000119`.
- [x] Remove the identity-less `PaymentService::create_refund` API.
- [x] Migrate payment service, controller, and migrated-schema fixtures to the
  owner idempotent refund service.
- [x] Add replay, conflicting-payload, same-key contention, and database hard-stop
  smoke tests.
- [x] Add and aggregate `verify-payment-refund-identity.mjs`.
- [x] Add opt-in feature `rustok-payment/stripe` with authorize, capture, cancel,
  refund, and webhook adapter source.
- [x] Keep Stripe credentials tenant-scoped through `StripeCredentialProvider`;
  static credentials are test/local-only.
- [x] Add source tests for Stripe HMAC verification, changed-body rejection, and
  minor-unit precision.
- [x] Classify provider configuration, unavailable, rejected, invalid-response,
  and unknown-outcome failures with typed `PaymentError` variants.
- [x] Route invalid/unknown external outcomes to
  `executing -> reconciliation_required` and forbid automatic re-claim.
- [x] Add migration `000120`, migrated-SQLite integration coverage, and
  `verify-payment-provider-outcome-contract.mjs` for uncertain outcomes.
- [x] Add opt-in server feature `payment-stripe` and compose Stripe into the shared
  payment provider registry used by GraphQL, REST, and native transports.
- [x] Resolve tenant Stripe credentials only through deployment-owned `SecretRef`
  mappings and `SecretResolverRegistry`; reject duplicate tenants, cross-tenant
  reference reuse, unknown resolver aliases, and raw secret configuration.
- [x] Add and aggregate `verify-payment-stripe-runtime.mjs`.
- [x] Update the legacy GraphQL runtime parity refund mutation helper to pass
  `idempotencyKey`.
- [x] Route REST refund completion and cancellation through
  `PaymentOrchestrationService` to preserve REST/GraphQL write parity.
- [ ] Execute deployment secret resolution and authorize, capture, cancel, refund,
  and webhook handling against a production-like Stripe endpoint.
- [ ] Prove adapters never persist payment/refund lifecycle state.

### Webhook ingress and durable inbox

- [x] Mount `POST /payment/webhooks/{provider_id}` through module codegen.
- [x] Require tenant scope, provider signature, non-empty body, and a 1 MiB limit.
- [x] Treat delivery/idempotency headers as optional untrusted hints.
- [x] Derive authoritative `delivery_id` and `replay_key` from the
  signature-verified provider result.
- [x] Reject hint conflicts before inbox insertion.
- [x] Persist only SHA-256 digest plus verified immutable normalized facts.
- [x] Enforce delivery/replay uniqueness, metadata limits, and immutable facts.
- [x] Apply payment/refund events only through owner services and mark processed
  after owner success.
- [x] Add static guards and tests for provider-verified identity.
- [ ] Execute real Stripe signature verification over mounted HTTP ingress.
- [ ] Retain malformed-signature, duplicate, unsupported, out-of-order,
  hint-conflict, and owner-conflict HTTP evidence.

### Recovery and dead-letter

- [x] Recover only received, failed, or expired-processing events.
- [x] Resume from durable normalized facts without raw body/provider parsing.
- [x] Isolate failures per event and dead-letter legacy rows without facts.
- [x] Exclude dead-letter from automatic retry.
- [x] Support operator-only
  `dead_letter -> processing -> processed | dead_letter`.
- [x] Require `payments:manage` and return safe projections.
- [x] Run bounded recovery in the shared server worker lifecycle, including already
  received events for inactive tenants.
- [ ] Execute restart, expired lease, concurrent replica, partial batch, and
  owner-apply-before-inbox-completion scenarios on PostgreSQL.
- [ ] Retain authenticated operator recovery/replay HTTP evidence.

Payment evidence:

- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`
- `crates/rustok-payment/docs/provider-webhooks.md`
- `crates/rustok-payment/src/providers.rs`
- `crates/rustok-payment/src/stripe_provider.rs`
- `crates/rustok-payment/src/services/refund_creation.rs`
- `crates/rustok-payment/src/migrations/m20260714_000118_enforce_refund_creation_identity.rs`
- `crates/rustok-payment/src/migrations/m20260714_000119_require_refund_creation_identity.rs`
- `crates/rustok-payment/src/migrations/m20260714_000120_allow_uncertain_provider_outcomes.rs`
- `crates/rustok-migrations/tests/refund_creation_identity_smoke.rs`
- `crates/rustok-migrations/tests/refund_creation_identity_required_smoke.rs`
- `crates/rustok-migrations/tests/payment_provider_operation_uncertain_outcome.rs`
- `apps/server/src/services/payment_provider_runtime.rs`
- `apps/server/src/services/commerce_provider_runtime.rs`
- `apps/server/tests/payment_refund_identity_guard.rs`
- `scripts/verify/verify-payment-refund-identity.mjs`
- `scripts/verify/verify-payment-provider-outcome-contract.mjs`
- `scripts/verify/verify-payment-stripe-runtime.mjs`
- `apps/server/src/services/payment_provider_event_worker.rs`

## Cross-domain evidence

- [x] Execute one compiled product/cart/inventory checkout validation proving
  channel-hidden inventory blocks checkout.
- [ ] Execute all product, cart, customer, region, pricing, inventory, order,
  payment, and fulfillment ports with real deadlines.
- [ ] Retain retryable, unavailable, degraded, timeout, malformed, and fallback
  evidence for every provider/consumer pair.
- [ ] Prove equivalent native, REST, and GraphQL behavior.
- [ ] Replace placeholder/static packets with observed evidence.

## Next capability phases

- [ ] Complete Pricing 2.0, stacking, temporal/channel/market rules, and
  deterministic explanations.
- [ ] Complete promotion/coupon ownership and adjustment attribution.
- [ ] Introduce explicit market/store configuration contracts.
- [ ] Complete return, exchange, claim, cancellation, refund, and fulfillment
  runtime evidence with durable identities.
- [ ] Execute the marketplace foundation gate in the order seller, membership,
  offer, order allocation, commission, ledger, payout, and surfaces.

## Verification and promotion checklist

Source inspection is not execution evidence.

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
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`

### Compile and tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-server --features payment-stripe,mod-commerce`
- [ ] `cargo xtask module test commerce`
- [ ] `cargo xtask module test payment`
- [ ] Targeted checkout, return-completion journal/command inbox, refund identity,
  provider-operation, provider-event, replay, recovery, and lifecycle tests.
- [ ] Stripe feature tests.

### Database and runtime

- [ ] Apply the clean SQLite graph and supported rollback/reapply paths.
- [ ] Apply the clean and upgraded PostgreSQL graph.
- [ ] Execute checkout/return-completion/refund/provider-operation/provider-event
  contention.
- [ ] Verify recovery/dead-letter query plans with production-like rows.
- [ ] Prove all declared routers are mounted.
- [ ] Exercise authenticated checkout, return-completion recovery, compensation,
  reconciliation, recovery, and replay.
- [ ] Prove workers obey runtime profile/shutdown and replicas cannot double-apply.
- [ ] Retain real payment signature, redelivery, degraded, reconciliation, and
  operator evidence.

## Immediate execution order

The capability and evidence tracks now proceed in parallel. Marketplace source
work must not wait for every external-adapter proof, and evidence work must not be
abandoned while marketplace capabilities are added.

1. [x] Add immutable return-completion command snapshots, atomic command/operation
   admission, and tenant-scoped operator list/show/retry routes.
2. [ ] Create `rustok-seller` with seller lifecycle and memberships.
3. [ ] Create `rustok-offer` with master-product linkage, approval, visibility, and
   deterministic eligibility projections.
4. [ ] Run static ecommerce/payment verifiers and fix remaining drift.
5. [ ] Run commerce, payment, Stripe-feature, and server compile checks.
6. [ ] Run clean SQLite migrations and targeted regression tests, including
   return-completion command replay and rollback/reapply.
7. [ ] Run PostgreSQL contention, restart, and kill-point scenarios for checkout,
   return completion, payment operations, and webhook recovery.
8. [ ] Introduce seller order allocations and commission snapshots only after the
   seller/offer contracts are source-guarded.
9. [ ] Build the double-entry ledger before payout commands or balances.
10. [ ] Execute deployment secret resolution and the Stripe adapter against a
    production-like endpoint.
11. [ ] Run mounted HTTP and background-worker recovery scenarios.
12. [ ] Integrate and execute an approved carrier adapter.
13. [ ] Reassess FFA/FBA promotion from retained evidence.

## Change rules

1. Update this file with every completed or newly discovered ecommerce task.
2. Do not maintain status checklists in owner runbooks, contracts, evidence JSON,
   issues, or chat-only plans.
3. Owner modules retain domain invariants, persistence, provider policy, and owner
   commands.
4. Never persist or expose raw provider payloads, signatures, SQL messages, or SDK
   errors.
5. Keep manifests, registries, OpenAPI, runbooks, and this plan aligned.
6. Update `docs/modules/registry.md` only when FFA/FBA status changes.

## References

- [Commerce crate README](../README.md)
- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Payment webhook runbook](../../rustok-payment/docs/provider-webhooks.md)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
