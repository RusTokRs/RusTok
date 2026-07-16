# RusToK ecommerce implementation plan

Last reviewed: 2026-07-16

## Source of truth

This file is the single human-maintained source of truth for ecommerce
implementation work, completion marks, verification state, execution order, and
promotion gates.

The ecommerce family includes `rustok-commerce` and the owner modules it
orchestrates: product, cart, customer, region, pricing, inventory, order, payment,
fulfillment, tax, promotion, market/store, seller/offer, commission, ledger, and
payout.

Rules:

- `[x]` means the source change or retained execution evidence is present in
  `main`.
- `[ ]` means implementation or required execution evidence remains outstanding.
- Source implementation and runtime verification are separate checklist items.
- A task is checked in the same change that lands its source or retained evidence.
- Owner-module runbooks and contracts describe behavior; they do not maintain a
  second roadmap or completion checklist.
- Newly discovered ecommerce work must be added here before or with its source
  change.

`rustok-commerce` is the orchestration root, not the owner of every ecommerce
aggregate. Owner modules retain their services, persistence, invariants, provider
policy, and domain UI. Commerce composes them through typed ports and durable
orchestration.

## Current boundary

- FFA status: `in_progress`.
- FBA status: `boundary_ready`.
- Structural shape: `core_transport_ui`.
- Payment FFA status: `in_progress`.
- Payment FBA status: `boundary_ready`.
- Source inspection and authored tests do not promote a boundary without compile,
  migration, transport, concurrency, and external-provider execution evidence.
- Commerce consumer registry:
  `crates/rustok-commerce/contracts/commerce-fba-registry.json`.
- Runtime provider invocation trace:
  `crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json`.
- Owner provider registries:
  - `crates/rustok-pricing/contracts/pricing-fba-registry.json`
  - `crates/rustok-inventory/contracts/inventory-fba-registry.json`
  - `crates/rustok-order/contracts/order-fba-registry.json`
  - `crates/rustok-payment/contracts/payment-fba-registry.json`
  - `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`
  - `crates/rustok-product/contracts/product-fba-registry.json`
  - `crates/rustok-customer/contracts/customer-fba-registry.json`
  - `crates/rustok-cart/contracts/cart-fba-registry.json`

## Architecture invariants

- [x] Keep checkout and cross-domain recovery orchestration in
  `rustok-commerce`.
- [x] Keep product, cart, customer, region, pricing, inventory, order, payment,
  and fulfillment persistence in their owner modules.
- [x] Call owner modules through typed ports or explicitly published owner runtime
  APIs instead of importing their persistence entities.
- [x] Carry tenant, actor, locale, channel, correlation, deadline, and idempotency
  context across owner calls.
- [x] Persist checkout operation identity, request/cart hashes, leases,
  checkpoints, and owner aggregate identities.
- [x] Use stable owner identities for replay instead of rediscovering side effects
  from mutable business state.
- [x] Keep raw provider payload parsing and signature verification out of
  `rustok-commerce`.
- [x] Keep payment/refund lifecycle persistence exclusively in
  `rustok-payment::PaymentService`.
- [x] Keep fulfillment lifecycle persistence and carrier policy in
  `rustok-fulfillment`.
- [x] Maintain this file as the only ecommerce task checklist; owner plan files may
  only redirect or document module-local behavior without completion marks.
- [x] Remove the duplicated legacy commerce status block and obsolete statements
  that contradicted the staged production runtime.
- [ ] Execute the complete provider-consumer graph with retained runtime evidence.

## Checkout orchestration workstream

### Checkout admission and immutable plan

- [x] Require a stable checkout idempotency key at public storefront boundaries.
- [x] Reuse the same key across UI and transport retries.
- [x] Route GraphQL, native, and REST checkout through staged checkout rather than
  the former monolithic production entrypoint.
- [x] Read and mutate cart state through cart-owned storefront and checkout ports.
- [x] Resolve product projections through `ProductCatalogReadPort` without
  importing product entities.
- [x] Resolve effective pricing through pricing-owned ports.
- [x] Validate channel, locale, region, country, shipping, product, pricing, and
  inventory context before external effects.
- [x] Build one immutable order/fulfillment plan before cross-domain side effects.
- [ ] Retain full native/REST/GraphQL parity evidence for checkout admission and
  validation failures.

### Durable stages and replay

- [x] Persist checkout operation identity, request hash, cart snapshot hash,
  execution lease, stage, error code, and owner aggregate ids.
- [x] Resume through `cart_locked`, `inventory_reserved`, `order_created`,
  `payment_ready`, `payment_authorized`, `payment_captured`,
  `fulfillment_created`, `cart_completed`, and `completed`.
- [x] Adopt existing inventory reservation identities on replay.
- [x] Prevent the legacy order-confirmation path from double-reserving checkout
  demand.
- [x] Accept already committed owner results for order, payment, fulfillment, and
  cart completion replay.
- [x] Prevent a second active checkout operation for the same cart.
- [x] Route REST, GraphQL, native storefront, and the historical journal wrapper
  through the same recovering staged runtime.
- [ ] Add and execute kill points after every owner call and before every
  checkpoint.
- [ ] Prove process restart does not duplicate reservations, orders, collections,
  provider operations, fulfillments, labels, or cart completion.

### Compensation and reconciliation

- [x] Separate pre-order reservation release from post-order cancellation.
- [x] Release adopted inventory through order lifecycle ownership.
- [x] Avoid automatic reversal of captured or uncertain provider effects.
- [x] Provide immediate safe compensation after staged checkout failure.
- [x] Provide a bounded lease-protected compensation sweep.
- [x] Classify manual financial reconciliation as
  `reconciliation_required` instead of an infinite compensation retry.
- [x] Exclude reconciliation-required operations from automatic compensation
  claims.
- [x] Block new checkout and payment provider execution while reconciliation is
  open.
- [x] Return transport-safe reconciliation errors without SQL/provider details.
- [x] Publish bounded admin operation reads, compensation commands, and sweep
  routes with `orders:manage`.
- [ ] Prove compensation/reconciliation contention and restart behavior on
  PostgreSQL.
- [ ] Execute complete operator compensation/reconciliation workflows over mounted
  HTTP routes.

Checkout evidence locations:

- `crates/rustok-commerce/src/services/staged_checkout.rs`
- `crates/rustok-commerce/src/services/checkout_stage_pipeline.rs`
- `crates/rustok-commerce/src/services/checkout_compensation.rs`
- `crates/rustok-commerce/src/services/recovering_staged_checkout.rs`
- `crates/rustok-commerce/src/controllers/store/mod.rs`
- `crates/rustok-commerce/storefront/src/transport/native_server_adapter.rs`
- `scripts/verify/verify-commerce-admin-boundary.mjs`
- `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`

## Payment workstream

This section replaces the former task checklist in
`crates/rustok-payment/docs/implementation-plan.md`. The payment file is only a
compatibility redirect to this plan.

### Payment ownership and checkout boundary

- [x] Keep payment collections, payments, refunds, and lifecycle transitions in
  `rustok-payment`.
- [x] Reference cart, order, and customer by identifier rather than owning their
  persistence state.
- [x] Keep `PaymentService` as the sole owner after provider operations and
  webhook normalization.
- [x] Publish `PaymentCollectionPort` with typed `PortContext` and `PortError`.
- [x] Enforce write idempotency for collection creation/reuse.
- [x] Keep native storefront transport host-neutral through
  `HostRuntimeContext`.
- [x] Retain GraphQL as fallback for `create_payment_collection`,
  `fetch_payment_collection`, and `fetch_refund_summary`.
- [x] Use `execute_selected_transport` for payment storefront transport
  selection.
- [x] Lock the storefront core/transport/UI split with
  `verify-payment-storefront-boundary.mjs`.
- [ ] Execute the checkout payment port through a real remote adapter.
- [ ] Retain timeout, typed-error, fallback, cart-ownership, and native/GraphQL
  parity evidence for the remote profile.

### Payment provider SPI and outbound operations

- [x] Publish provider descriptors, capabilities, health, degraded mode, and
  registration validation.
- [x] Keep the built-in manual provider as a baseline adapter.
- [x] Guard authorize, capture, cancel, refund, and webhook operations through
  `PaymentProviderRegistry`.
- [x] Reject missing, unavailable, unsupported, unknown, mismatched, and duplicate
  provider registrations before adapter invocation.
- [x] Preserve idempotency context across provider requests.
- [x] Persist payment-owned provider-operation execution and reconciliation state.
- [x] Use compare-and-set claims and explicit reconciliation-required outcomes for
  external side effects.
- [ ] Implement and register an approved production gateway adapter.
- [ ] Configure gateway credentials through deployment-owned secret management.
- [ ] Exercise authorize, capture, cancel, and refund against a production-like
  gateway while proving the adapter never persists lifecycle state.

### Payment webhook ingress and durable inbox

- [x] Mount `POST /payment/webhooks/{provider_id}` through module codegen.
- [x] Enforce tenant scope, delivery identity, idempotency identity, supported
  signature headers, non-empty body, and a 1 MiB body limit.
- [x] Invoke provider-owned cryptographic verification and normalization before
  inbox insertion.
- [x] Persist only a SHA-256 payload digest; never persist or log the raw body or
  signature.
- [x] Persist verified normalized facts atomically with the first inbox receipt.
- [x] Enforce delivery and idempotency uniqueness per tenant/provider.
- [x] Reject identity reuse with another payload digest or normalized event.
- [x] Bound normalized metadata to 64 KiB and depth 16.
- [x] Protect normalized event type, external reference, and metadata from later
  mutation with database guards.
- [x] Apply `payment.authorized`, `payment.captured`, `payment.cancelled`, and
  `refund.completed` only through owner services.
- [x] Mark an inbox event `processed` only after the owner transition succeeds.
- [x] Use hash-only terminology in payment registry, evidence packets, verifier,
  and verifier fixtures.
- [ ] Execute signature verification with a concrete external provider.
- [ ] Retain malformed signature, duplicate delivery, unsupported event,
  out-of-order capture, and owner-conflict HTTP evidence.

### Payment recovery and dead-letter operations

- [x] Claim only `received`, `failed`, or expired `processing` events for automatic
  recovery.
- [x] Resume from durable normalized facts without provider reparsing or raw body
  access.
- [x] Isolate recovery errors per event so one row cannot abort a batch.
- [x] Move legacy rows without normalized facts to `dead_letter`.
- [x] Exclude `dead_letter` from automatic retry.
- [x] Support explicit
  `dead_letter -> processing -> processed | dead_letter` replay.
- [x] Require `payments:manage` for recovery and dead-letter replay.
- [x] Return safe operator projections without digest, idempotency key, metadata,
  lease details, raw body/signature, or internal error text.
- [x] Publish tenant-scoped bounded recovery and replay endpoints.
- [x] Run bounded recovery in the standard server background-worker lifecycle.
- [x] Reuse the shared shutdown handle and prevent duplicate startup in one
  process.
- [x] Continue recovery for already-received financial events of inactive tenants
  without reopening user traffic.
- [ ] Execute worker restart, expired lease, concurrent replica, partial batch,
  and owner-apply-before-inbox-completion scenarios on PostgreSQL.
- [ ] Retain authenticated operator recovery and replay HTTP evidence.

### Payment host integration and authored tests

- [x] Gate Axum and OpenAPI dependencies behind the payment `server` feature.
- [x] Enable `rustok-payment/server` from server feature `mod-payment`.
- [x] Compose payment and webhook routers through `rustok-module.toml`.
- [x] Preserve the host-registered provider registry across transports.
- [x] Publish ingress, operator reads, recovery, and replay in OpenAPI.
- [x] Align payment manifest, payment registry, commerce consumer registry,
  webhook contract, runbook, and provider evidence terminology.
- [x] Author regression coverage for inbox deduplication, payload collisions,
  leases, retry, completion, atomic receipt, normalized-fact immutability,
  dead-letter exclusion, operator replay, durable recovery, legacy rows, and
  payment/refund lifecycle replay.
- [ ] Execute the authored payment tests in a dependency-complete environment.
- [ ] Retain test output instead of source-only evidence.

Payment evidence locations:

- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`
- `crates/rustok-payment/docs/provider-webhooks.md`
- `crates/rustok-payment/src/providers.rs`
- `crates/rustok-payment/src/services/provider_operation.rs`
- `crates/rustok-payment/src/services/provider_event.rs`
- `crates/rustok-payment/src/services/provider_event_ingress.rs`
- `crates/rustok-payment/src/services/provider_event_recovery.rs`
- `apps/server/src/services/payment_provider_event_worker.rs`
- payment provider-event migrations under
  `crates/rustok-payment/src/migrations/`
- payment migration smoke tests under `crates/rustok-migrations/tests/`

## Cross-domain provider and transport evidence

- [x] Execute one compiled product/cart/inventory checkout validation path proving
  channel-hidden inventory blocks checkout.
- [ ] Execute all declared product, cart, customer, region, pricing, inventory,
  order, payment, and fulfillment ports with real deadlines.
- [ ] Retain retryable, unavailable, degraded, timeout, malformed-response, and
  fallback evidence for each provider/consumer pair.
- [ ] Prove consistent native, REST, and GraphQL behavior for equivalent commands.
- [ ] Replace placeholder/static provider evidence with observed execution packets.
- [ ] Promote boundaries only after the required execution evidence exists.

## Next ecommerce capability phases

These phases remain behind the staged-checkout correctness and live-evidence gate.

### Pricing, promotion, and market/store

- [ ] Complete Pricing 2.0 rule semantics, stacking policy, temporal validity,
  channel/market targeting, and deterministic explanations.
- [ ] Complete promotion ownership, coupon lifecycle, usage limits, and adjustment
  attribution without embedding promotion persistence in commerce.
- [ ] Introduce explicit market/store configuration and channel/region/currency
  resolution contracts.

### Post-order and returns

- [ ] Complete return, exchange, claim, cancellation, refund, and fulfillment
  orchestration through owner services.
- [ ] Persist operation journals and idempotency identities for every external
  post-order effect.
- [ ] Add reconciliation and operator recovery for uncertain refund, inventory,
  and fulfillment outcomes.

### Marketplace, ledger, and payouts

- [ ] Introduce seller, offer, commission, and settlement ownership as separate
  bounded contexts.
- [ ] Build an immutable double-entry ledger before implementing payouts.
- [ ] Derive seller balances and payout eligibility from ledger entries, never from
  mutable order totals.
- [ ] Implement payout provider journals, retries, reconciliation, and audit
  evidence.

## Verification and promotion checklist

Unchecked commands remain unverified until actually executed. Source inspection is
not execution evidence.

### Static verification

- [ ] `cargo fmt --all -- --check`
- [ ] `npm run verify:ecommerce:fba`
- [ ] `npm run verify:ecommerce:provider-spi-evidence`
- [ ] `npm run verify:commerce:admin-boundary`
- [ ] `npm run verify:commerce:storefront-transport-handoff`
- [ ] `npm run verify:payment:storefront-boundary`
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`

### Compile and tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-server --features mod-payment`
- [ ] `cargo xtask module test commerce`
- [ ] `cargo xtask module test payment`
- [ ] Targeted checkout, inventory reservation, payment provider-operation,
  provider-event, replay, recovery, and lifecycle tests.

### Database and concurrency

- [ ] Apply the clean ecommerce migration graph to SQLite.
- [ ] Exercise supported migration rollback/reapply paths.
- [ ] Apply the clean and upgraded migration graph to PostgreSQL.
- [ ] Execute checkout lease, compensation lease, provider-operation claim,
  provider-event claim, duplicate delivery, payload collision, immutable facts,
  and dead-letter replay contention.
- [ ] Verify recovery/dead-letter indexes and query plans with production-like row
  counts.

### Runtime and external providers

- [ ] Start the complete ecommerce server profile and prove every declared router
  is mounted.
- [ ] Exercise authenticated checkout, compensation, reconciliation, payment
  recovery, and replay endpoints.
- [ ] Prove background workers start only in worker-enabled profiles and shut down
  through shared handles.
- [ ] Prove two replicas cannot apply the same checkout/payment effect twice.
- [ ] Integrate an approved payment gateway and carrier adapter.
- [ ] Retain real signature verification, redelivery, retry, degraded,
  unavailable, reconciliation, and operator-replay evidence.

## Immediate execution order

1. [ ] Run static ecommerce/payment verifiers and fix source/registry drift.
2. [ ] Run commerce, payment, and server compile checks.
3. [ ] Run clean SQLite migrations and targeted regression tests.
4. [ ] Run PostgreSQL migrations, contention, restart, and kill-point scenarios.
5. [ ] Run mounted HTTP and background-worker recovery scenarios.
6. [ ] Integrate and execute an approved production-like payment gateway.
7. [ ] Integrate and execute an approved carrier adapter.
8. [ ] Reassess FFA/FBA promotion using retained runtime evidence.
9. [ ] Continue Pricing 2.0, promotion, market/store, post-order, marketplace,
   ledger, and payout phases.

## Change rules

1. Update this file in the same commit as any completed or newly discovered
   ecommerce task.
2. Do not maintain status checklists in owner runbooks, contracts, evidence JSON,
   issues, or chat-only/local plans.
3. Owner modules keep domain invariants, persistence, provider policy, and owner
   commands even though their work is tracked here.
4. Never persist or expose raw payment provider payloads, signatures, SQL messages,
   or provider SDK errors.
5. Keep module manifests, owner/consumer registries, OpenAPI, runbooks, and this
   plan aligned with every public boundary change.
6. Update `docs/modules/registry.md` only when an FFA/FBA boundary status changes.

## References

- [Commerce crate README](../README.md)
- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Payment webhook runbook](../../rustok-payment/docs/provider-webhooks.md)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
