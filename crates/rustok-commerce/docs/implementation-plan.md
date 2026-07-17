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

`rustok-commerce` owns general ecommerce cross-domain orchestration. Product, cart,
customer, region, pricing, inventory, order, payment, fulfillment, tax, promotion,
and market/store remain owner bounded contexts. Marketplace capabilities belong to
the explicit `rustok-marketplace-*` family; seller, listing, commission, ledger, and
payout persistence must never be folded into `rustok-commerce`.

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
- Marketplace production promotion gate: `closed` until compile, migration,
  contention, restart, mounted transport, and owner/provider-consumer evidence is
  retained.
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
- [x] `crates/rustok-marketplace/contracts/marketplace-fba-registry.json`
- [x] `crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json`
- [x] `apps/server/tests/marketplace_family_boundary_guard.rs`
- [x] `scripts/verify/verify-marketplace-family-boundary.mjs`

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

## Marketplace Family

Marketplace owner-domain implementation may proceed in parallel with ecommerce
runtime evidence. It must not weaken checkout/payment/return boundaries and must
not be promoted as production-ready before the marketplace promotion gates close.

### Family naming and ownership

- [x] Create `rustok-marketplace` as the family root and cross-marketplace
  orchestration/composition boundary. It owns no seller, listing, commission,
  ledger, or payout tables.
- [ ] Create the remaining owner modules using the mandatory family prefix:
  `rustok-marketplace-listing`, `rustok-marketplace-commission`,
  `rustok-marketplace-ledger`, and `rustok-marketplace-payout`.
- [x] Create `rustok-marketplace-seller` as the first family owner module.
- [x] Keep master catalog in `rustok-product`, price/rules in `rustok-pricing`, stock
  in `rustok-inventory`, customer order lifecycle in `rustok-order`, provider
  payment/refund state in `rustok-payment`, and general ecommerce orchestration in
  `rustok-commerce`.
- [x] Forbid generic crate/module names such as `rustok-seller`, `rustok-offer`,
  `rustok-listing`, `rustok-commission`, `rustok-ledger`, or `rustok-payout` for
  marketplace-owned capabilities.
- [x] Register `marketplace_seller` and `marketplace` in `modules.toml` without
  enabling them by default before compile/migration evidence exists.

### FFA/FBA family contract

- [x] Give the implemented marketplace root and seller owner modules local
  `docs/implementation-plan.md` files with FFA/FBA status, structural shape,
  ownership, ports, and promotion evidence.
- [x] Declare root consumer and seller provider boundaries in
  `rustok-module.toml` plus module-local FBA registries using `PortContext`,
  `PortError`, deadlines, typed retryability, and idempotency admission.
- [x] Keep the in-process seller provider behind the same typed read/command port
  contracts expected by future remote adapters; the root consumer imports no
  seller entities or database connection.
- [x] Publish module-owned FFA package structure under
  `rustok-marketplace-seller/admin`; host applications are not yet allowed to own
  seller policy or fabricate seller data when transport is unmounted.
- [x] Use the structural sequence `core_only → core_transport → core_transport_ui`;
  do not raise FFA to `phase_b_ready` or FBA to `transport_verified` without
  retained compile and mounted runtime evidence.
- [x] Add source guards for family naming, root non-ownership, seller schema,
  manifests, FBA registries, explicit FFA transport selection, and host
  non-ownership.
- [ ] Aggregate the marketplace verifier into package scripts and execute it.
- [ ] Synchronize the central FFA/FBA readiness board after a safe full-file update;
  local plans remain authoritative meanwhile.

### Seller and membership

- [x] Create `rustok-marketplace-seller` as the owner of seller identity,
  legal/display profile, onboarding state, suspension state, and seller lifecycle
  transitions.
- [x] Add seller memberships with immutable seller scope, typed roles/status,
  tenant isolation, and atomic initial owner membership; platform RBAC controls
  seller administration access while membership policy remains seller-owned.
- [x] Publish `MarketplaceSellerReadPort` and `MarketplaceSellerCommandPort` with
  deadline, actor/tenant context, safe typed errors, and required command
  idempotency keys.
- [ ] Persist durable seller command receipts and conflicting-payload detection so
  idempotency survives a successful owner write followed by a lost response.
- [ ] Add seller onboarding review/audit events; keep KYC provider details behind a
  provider SPI and persist only normalized verification facts.
- [x] Publish the first FFA source slice in `rustok-marketplace-seller/admin` with
  framework-neutral core/model, explicit native/GraphQL profile selection,
  transport facade, i18n catalogs, and a thin Leptos adapter.
- [ ] Mount authenticated native and GraphQL seller transports and implement full
  list/detail/onboarding/suspension/member workflows.

Seller evidence:

- `crates/rustok-marketplace-seller/src/entities/seller.rs`
- `crates/rustok-marketplace-seller/src/entities/seller_member.rs`
- `crates/rustok-marketplace-seller/src/service.rs`
- `crates/rustok-marketplace-seller/src/ports.rs`
- `crates/rustok-marketplace-seller/src/migrations/m20260716_000001_create_marketplace_sellers.rs`
- `crates/rustok-marketplace-seller/rustok-module.toml`
- `crates/rustok-marketplace-seller/contracts/marketplace-seller-fba-registry.json`
- `crates/rustok-marketplace-seller/admin/src/core.rs`
- `crates/rustok-marketplace-seller/admin/src/transport.rs`
- `crates/rustok-marketplace-seller/admin/src/ui/leptos.rs`
- `apps/server/tests/marketplace_family_boundary_guard.rs`
- `scripts/verify/verify-marketplace-family-boundary.mjs`

### Master catalog and listings

- [ ] Create `rustok-marketplace-listing` as the owner of seller listings linked to
  product-owned master products/variants; do not copy canonical product content
  into marketplace tables.
- [ ] Model listing status, seller SKU, pricing reference, inventory reference,
  fulfillment profile, market/channel visibility, publication, and approval.
- [ ] Enforce one active seller listing identity per seller/master variant/market
  scope while allowing versioned commercial terms.
- [ ] Publish deterministic listing eligibility and selection projections for cart,
  pricing, inventory, search, and storefront consumers.
- [ ] Add product matching/approval workflows before automatic EAN/GTIN matching,
  deduplication, or buy-box ranking.

### Marketplace order ownership

- [ ] Introduce durable order groups and seller allocations without duplicating the
  customer order aggregate.
- [ ] Snapshot seller, listing, commission policy, fulfillment ownership, and
  monetary allocation on order lines at checkout.
- [ ] Route seller-specific fulfillment, cancellation, return, claim, and refund
  decisions through commerce/marketplace orchestration and owner commands.
- [ ] Prevent one seller's lifecycle operation from mutating another seller's
  allocation or financial state.

### Commission, ledger, and payout

- [ ] Create `rustok-marketplace-commission` with versioned policies and
  deterministic calculation explanations; snapshot the applied policy/result on
  order allocations.
- [ ] Create immutable double-entry `rustok-marketplace-ledger` before implementing
  balances or payouts.
- [ ] Derive pending, available, reserved, disputed, and paid seller balances only
  from ledger entries.
- [ ] Create `rustok-marketplace-payout` with idempotent payout journals, provider
  SPI, retries, reconciliation, reversals, and operator audit.
- [ ] Keep split-payment provider capabilities optional; internal allocation and
  ledger correctness must not depend on a specific PSP.

### Marketplace surfaces and advanced capabilities

- [ ] Build vendor portal and platform-admin transports over marketplace seller,
  listing, allocation, ledger, and payout owner APIs; UI must not own marketplace
  policy.
- [ ] Add storefront multi-seller listing display and deterministic selection before
  implementing buy-box ranking.
- [ ] Add multi-channel stock sync, KYC adapters, automated catalog matching, and
  PSP split payouts only after the corresponding owner contracts and recovery
  journals are proven.

### Marketplace promotion gates

- [ ] FBA `boundary_ready`: typed provider/consumer ports, manifest declarations,
  in-process provider, registry, contract cases, durable command identity/error/
  deadline rules, and source guards exist.
- [ ] FBA `transport_verified`: compiled in-process and remote-profile contract
  evidence, timeout/degraded/fallback behavior, and mounted consumer execution are
  retained.
- [ ] FFA `phase_b_ready`: module-owned core, transport facade, Leptos adapter,
  i18n, host composition, and native/GraphQL parity evidence are retained.
- [ ] FFA `parity_verified`: supported hosts render equivalent workflows and error
  states without importing owner internals.
- [ ] Marketplace production-ready: clean/upgraded migrations, tenant isolation,
  contention/restart recovery, mounted admin/vendor/storefront flows, and ledger/
  payout reconciliation evidence are retained.

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
  payment, fulfillment, and marketplace ports with real deadlines.
- [ ] Retain retryable, unavailable, degraded, timeout, malformed, and fallback
  evidence for every provider/consumer pair.
- [ ] Prove equivalent native, REST, GraphQL, and module-owned FFA behavior.
- [ ] Replace placeholder/static packets with observed evidence.

## Next capability phases

- [ ] Complete Pricing 2.0, stacking, temporal/channel/market rules, and
  deterministic explanations.
- [ ] Complete promotion/coupon ownership and adjustment attribution.
- [ ] Introduce explicit market/store configuration contracts.
- [ ] Complete return, exchange, claim, cancellation, refund, and fulfillment
  runtime evidence with durable identities.
- [ ] Execute the Marketplace Family in the order listing, order allocation,
  commission, ledger, payout, and full surfaces after seller command identity and
  real transports are closed.

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
- [ ] `node scripts/verify/verify-marketplace-family-boundary.mjs`
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate payment`
- [ ] `cargo xtask module validate marketplace`
- [ ] `cargo xtask module validate marketplace_seller`

### Compile and tests

- [ ] `cargo check -p rustok-commerce --lib`
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-marketplace --lib`
- [ ] `cargo check -p rustok-marketplace-seller --lib`
- [ ] `cargo check -p rustok-marketplace-seller-admin --all-features`
- [ ] `cargo check -p rustok-server --features payment-stripe,mod-commerce`
- [ ] `cargo xtask module test commerce`
- [ ] `cargo xtask module test payment`
- [ ] Targeted checkout, return-completion journal/command inbox, refund identity,
  provider-operation, provider-event, marketplace seller lifecycle, membership,
  port contract, replay, recovery, and tenant-isolation tests.
- [ ] Stripe feature tests.

### Database and runtime

- [ ] Apply the clean SQLite graph and supported rollback/reapply paths.
- [ ] Apply the clean and upgraded PostgreSQL graph.
- [ ] Execute checkout/return-completion/refund/provider-operation/provider-event
  contention.
- [ ] Execute marketplace seller/membership tenant isolation and concurrent command
  scenarios.
- [ ] Verify recovery/dead-letter query plans with production-like rows.
- [ ] Prove all declared routers and module-owned UI packages are mounted.
- [ ] Exercise authenticated checkout, return-completion recovery, compensation,
  marketplace seller administration, reconciliation, recovery, and replay.
- [ ] Prove workers obey runtime profile/shutdown and replicas cannot double-apply.
- [ ] Retain real payment signature, redelivery, degraded, reconciliation, and
  operator evidence.

## Immediate execution order

The capability and evidence tracks proceed in parallel. Marketplace source work
must not wait for every external-adapter proof, and evidence work must not be
abandoned while marketplace capabilities are added.

1. [x] Add immutable return-completion command snapshots, atomic command/operation
   admission, and tenant-scoped operator list/show/retry routes.
2. [x] Create `rustok-marketplace` family root with module manifest, FBA consumer
   registry, and typed seller directory consumer.
3. [x] Create `rustok-marketplace-seller` with seller lifecycle, memberships,
   typed FBA provider ports, migrations, workspace/module registration, and source
   guards.
4. [x] Create `rustok-marketplace-seller/admin` with module-owned FFA core/model,
   explicit transport facade, Leptos adapter, and i18n catalogs.
5. [ ] Add durable seller command receipts and conflicting-payload replay guards.
6. [ ] Mount native/GraphQL seller transports and finish seller admin workflows.
7. [ ] Create `rustok-marketplace-listing` with master-product linkage, approval,
   visibility, and deterministic eligibility projections.
8. [ ] Run static ecommerce/payment/marketplace verifiers and fix remaining drift.
9. [ ] Run commerce, payment, marketplace, Stripe-feature, and server compile checks.
10. [ ] Run clean SQLite migrations and targeted regression tests, including
    return-completion command replay and marketplace seller tenant isolation.
11. [ ] Run PostgreSQL contention, restart, and kill-point scenarios for checkout,
    return completion, payment operations, webhook recovery, and marketplace writes.
12. [ ] Introduce seller order allocations and commission snapshots only after the
    seller/listing contracts are source-guarded.
13. [ ] Build the double-entry marketplace ledger before payout commands or balances.
14. [ ] Execute deployment secret resolution and the Stripe adapter against a
    production-like endpoint.
15. [ ] Run mounted HTTP and background-worker recovery scenarios.
16. [ ] Integrate and execute an approved carrier adapter.
17. [ ] Reassess FFA/FBA promotion from retained evidence.

## Change rules

1. Update this file with every completed or newly discovered ecommerce task.
2. Do not maintain status checklists in owner runbooks, contracts, evidence JSON,
   issues, or chat-only plans.
3. Owner modules retain domain invariants, persistence, provider policy, and owner
   commands.
4. Never persist or expose raw provider payloads, signatures, SQL messages, SDK
   errors, or KYC provider payloads.
5. Keep manifests, registries, OpenAPI, runbooks, and this plan aligned.
6. Update `docs/modules/registry.md` whenever a local FFA/FBA status changes.
7. Marketplace capability crates and module slugs must preserve the
   `rustok-marketplace-*` / `marketplace_*` family identity.

## References

- [Commerce crate README](../README.md)
- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Payment webhook runbook](../../rustok-payment/docs/provider-webhooks.md)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
- [Marketplace root plan](../../rustok-marketplace/docs/implementation-plan.md)
- [Marketplace seller plan](../../rustok-marketplace-seller/docs/implementation-plan.md)
