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

Checkout evidence:

- `crates/rustok-commerce/src/services/staged_checkout.rs`
- `crates/rustok-commerce/src/services/checkout_stage_pipeline.rs`
- `crates/rustok-commerce/src/services/checkout_compensation.rs`
- `crates/rustok-commerce/src/services/recovering_staged_checkout.rs`
- `crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs`
- `scripts/verify/verify-commerce-admin-boundary.mjs`
- `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`

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
- [ ] Update the legacy GraphQL runtime parity refund mutation helper to pass
  `idempotencyKey`.
- [ ] Replace generic validation errors with typed provider unavailable/rejected/
  invalid-response classifications.
- [ ] Register Stripe through a deployment-owned tenant secret resolver.
- [ ] Compile and execute authorize, capture, cancel, and refund against a
  production-like Stripe endpoint.
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
- `crates/rustok-migrations/tests/refund_creation_identity_smoke.rs`
- `crates/rustok-migrations/tests/refund_creation_identity_required_smoke.rs`
- `scripts/verify/verify-payment-refund-identity.mjs`
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
  orchestration with durable identities.
- [ ] Introduce seller, offer, commission, and settlement bounded contexts.
- [ ] Build immutable double-entry ledger before payouts.
- [ ] Derive balances and payout eligibility from ledger entries.
- [ ] Implement payout journals, retries, reconciliation, and audit evidence.

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
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`

### Compile and tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-server --features mod-payment`
- [ ] `cargo xtask module test commerce`
- [ ] `cargo xtask module test payment`
- [ ] Targeted checkout, refund identity, provider-operation, provider-event,
  replay, recovery, and lifecycle tests.
- [ ] Stripe feature tests.

### Database and runtime

- [ ] Apply the clean SQLite graph and supported rollback/reapply paths.
- [ ] Apply the clean and upgraded PostgreSQL graph.
- [ ] Execute checkout/refund/provider-operation/provider-event contention.
- [ ] Verify recovery/dead-letter query plans with production-like rows.
- [ ] Prove all declared routers are mounted.
- [ ] Exercise authenticated checkout, compensation, reconciliation, recovery, and
  replay.
- [ ] Prove workers obey runtime profile/shutdown and replicas cannot double-apply.
- [ ] Retain real payment signature, redelivery, degraded, reconciliation, and
  operator evidence.

## Immediate execution order

1. [ ] Update the GraphQL runtime parity refund helper with `idempotencyKey`.
2. [ ] Run static ecommerce/payment verifiers and fix remaining drift.
3. [ ] Run commerce, payment, Stripe-feature, and server compile checks.
4. [ ] Run clean SQLite migrations and targeted regression tests.
5. [ ] Run PostgreSQL contention, restart, and kill-point scenarios.
6. [ ] Mount a deployment-owned Stripe tenant credential resolver and execute the
   adapter against a production-like endpoint.
7. [ ] Run mounted HTTP and background-worker recovery scenarios.
8. [ ] Integrate and execute an approved carrier adapter.
9. [ ] Reassess FFA/FBA promotion from retained evidence.

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
