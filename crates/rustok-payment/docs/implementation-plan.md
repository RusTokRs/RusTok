# Implementation plan for `rustok-payment`

Last reviewed: 2026-07-14

## Source of truth

This file is the single source of truth for `rustok-payment` implementation work,
completion status, verification status, and promotion gates.

- `[x]` means the source change is present in `main`.
- `[ ]` means the task or its required execution evidence is still outstanding.
- Source implementation and runtime verification are tracked as separate tasks.
- A task is checked only in the same change that lands its implementation or
  retained evidence.
- `docs/provider-webhooks.md` is an operational runbook, not a second roadmap.
- Contract registries and evidence packets describe machine-readable boundaries;
  they must link back here instead of maintaining independent task lists.

## Current boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract:
  `payment.checkout.v1+provider_spi.v1+provider_webhook_inbox.v1`
- Promotion ceiling: source implementation does not promote the module beyond
  `boundary_ready` without compile, migration, transport, concurrency, and
  external-provider execution evidence.

`rustok-payment` owns payment collections, payments, refunds, provider policy,
provider-operation reconciliation, signature-verified webhook ingress, durable
provider-event inbox state, retry recovery, and operator replay. `PaymentService`
remains the sole owner of persisted payment/refund lifecycle transitions.
`rustok-commerce` composes checkout and must not parse provider webhook payloads,
persist payment lifecycle state, or introduce a second payment-service facade.

## Implementation checklist

### A. Ownership and domain boundary

- [x] Keep payment collections, payments, refunds, and lifecycle transitions in
  `rustok-payment`.
- [x] Reference cart, order, and customer records by identifier instead of
  importing their persistence models as payment-owned state.
- [x] Keep `PaymentService` as the lifecycle owner after provider operations and
  webhook normalization.
- [x] Keep checkout orchestration in `rustok-commerce` without provider payload
  parsing or direct payment-state persistence.
- [x] Publish module permissions and migrations through `PaymentModule`.

Evidence:

- `crates/rustok-payment/src/lib.rs`
- `crates/rustok-payment/src/services/payment.rs`
- `crates/rustok-payment/src/services/provider_event_domain.rs`
- `crates/rustok-commerce/src/services/checkout.rs`
- `crates/rustok-commerce/src/services/payment_orchestration.rs`

### B. Storefront and FBA checkout boundary

- [x] Publish `PaymentCollectionPort` with typed `PortContext`/`PortError`
  semantics.
- [x] Enforce write idempotency for collection creation/reuse and read policy for
  collection status.
- [x] Keep native storefront transport host-neutral through
  `HostRuntimeContext`.
- [x] Retain GraphQL as the selected fallback for collection creation, collection
  reads, and refund summaries.
- [x] Lock the storefront core/transport/UI split with the payment storefront
  verifier.
- [ ] Execute the checkout port contract through a real remote adapter.
- [ ] Retain timeout, typed-error, fallback, cart-ownership, and native/GraphQL
  parity evidence for the remote profile.

Done when the published checkout profiles have executable in-process and remote
adapter evidence rather than placeholder-only metadata.

Evidence:

- `crates/rustok-payment/src/ports.rs`
- `crates/rustok-payment/contracts/payment-fba-registry.json`
- `scripts/verify/verify-payment-storefront-boundary.mjs`

### C. Provider SPI and outbound operation safety

- [x] Publish provider descriptors, capabilities, health, degraded mode, and
  registration validation.
- [x] Keep the built-in manual provider as a baseline adapter.
- [x] Guard authorize, capture, cancel, refund, and webhook execution through
  `PaymentProviderRegistry`.
- [x] Reject unavailable, unsupported, unknown, or mismatched provider operations
  before adapter invocation.
- [x] Preserve idempotency context across provider operation requests.
- [x] Persist provider-operation execution and reconciliation state in the
  payment-owned journal.
- [x] Use compare-and-set claims and explicit reconciliation-required outcomes for
  external side effects.
- [ ] Implement and register a production gateway adapter.
- [ ] Configure gateway credentials and secrets through deployment-owned secret
  management.
- [ ] Exercise authorize, capture, cancel, and refund against a production-like
  gateway while proving that adapters do not persist lifecycle state.

Done when a concrete external gateway passes the guarded operation contract and
`PaymentService` remains the only lifecycle persistence owner.

Evidence:

- `crates/rustok-payment/src/providers.rs`
- `crates/rustok-payment/src/services/provider_operation.rs`
- `crates/rustok-payment/migrations/m20260713_000110_create_provider_operation_journal.rs`
- `crates/rustok-payment/migrations/m20260713_000111_enforce_provider_operation_lifecycle.rs`
- `crates/rustok-payment/migrations/m20260713_000112_claim_provider_operation_execution.rs`

### D. Webhook ingress and durable inbox

- [x] Mount `POST /payment/webhooks/{provider_id}` through module codegen.
- [x] Enforce tenant scope, delivery identity, idempotency identity, supported
  signature headers, non-empty body, and a 1 MiB body limit.
- [x] Invoke provider-owned cryptographic verification and normalization before
  inbox insertion.
- [x] Persist only a SHA-256 payload digest; never persist or log the raw provider
  body or signature.
- [x] Atomically persist verified normalized facts with the first inbox receipt.
- [x] Enforce delivery and idempotency uniqueness per tenant/provider.
- [x] Reject identity reuse with a different payload digest or different
  normalized facts.
- [x] Enforce bounded normalized metadata size and depth.
- [x] Protect normalized event type, external reference, and metadata from later
  mutation with database guards.
- [x] Apply payment and refund events only through owner services.
- [x] Mark an inbox event `processed` only after the owner transition succeeds.
- [ ] Execute webhook signature verification with a concrete external provider.
- [ ] Retain duplicate delivery, malformed signature, unsupported event, and
  out-of-order lifecycle HTTP evidence.

Evidence:

- `crates/rustok-payment/src/controllers.rs`
- `crates/rustok-payment/src/services/provider_event.rs`
- `crates/rustok-payment/src/services/provider_event_ingress.rs`
- `crates/rustok-payment/src/services/provider_event_lifecycle.rs`
- `crates/rustok-payment/src/services/provider_event_refund.rs`
- `crates/rustok-payment/contracts/payment-provider-webhook-v1.json`

### E. Retry recovery and dead-letter operations

- [x] Claim only `received`, `failed`, or expired `processing` events for automatic
  recovery.
- [x] Resume processing from durable verified normalized facts without provider
  reparsing or raw payload access.
- [x] Isolate recovery failures per event so one event cannot abort the batch.
- [x] Move legacy rows without normalized facts to `dead_letter` instead of
  retrying forever.
- [x] Exclude `dead_letter` rows from automatic retry.
- [x] Support explicit `dead_letter -> processing -> processed | dead_letter`
  operator replay.
- [x] Require `payments:manage` for recovery and dead-letter replay.
- [x] Return a safe operator projection without payload digest, idempotency key,
  normalized metadata, lease owner, signature, raw body, or internal error text.
- [x] Publish a bounded tenant-scoped recovery endpoint.
- [x] Run bounded recovery from the standard server background-worker lifecycle.
- [x] Reuse the shared shutdown handle and prevent duplicate worker startup in one
  process.
- [x] Recover already-received financial events for inactive tenants while keeping
  normal user traffic disabled.
- [ ] Execute worker restart, expired lease, concurrent replica, and partial batch
  failure scenarios against PostgreSQL.
- [ ] Retain operator recovery and dead-letter replay HTTP evidence.

Evidence:

- `crates/rustok-payment/src/services/provider_event_recovery.rs`
- `crates/rustok-payment/src/provider_event_recovery_controller.rs`
- `apps/server/src/services/payment_provider_event_worker.rs`
- `apps/server/src/services/module_event_dispatcher.rs`
- `crates/rustok-payment/migrations/m20260714_000116_allow_provider_event_replay.rs`
- `crates/rustok-payment/migrations/m20260714_000117_lock_provider_event_normalized_facts.rs`

### F. Host integration, contracts, and documentation

- [x] Gate Axum/OpenAPI dependencies behind the payment `server` feature.
- [x] Enable `rustok-payment/server` from the server `mod-payment` feature.
- [x] Compose normal and webhook routers through `rustok-module.toml`.
- [x] Preserve host-registered payment provider registries across all transports.
- [x] Publish ingress, operator reads, recovery, and replay in OpenAPI.
- [x] Align the payment module manifest, payment FBA registry, commerce consumer
  registry, webhook contract, and provider runbook with the implemented source
  boundary.
- [x] Make this file the only human-maintained implementation status checklist.
- [ ] Replace the legacy payment evidence assertion
  `raw_payload_retained_for_audit` with hash-only terminology in the shared
  provider evidence verifier and fixtures.
- [ ] Add a verifier rule that rejects a second payment roadmap/checklist outside
  this file.

Evidence:

- `crates/rustok-payment/Cargo.toml`
- `crates/rustok-payment/rustok-module.toml`
- `crates/rustok-payment/src/openapi.rs`
- `apps/server/Cargo.toml`
- `apps/server/src/services/commerce_provider_runtime.rs`
- `crates/rustok-commerce/contracts/commerce-fba-registry.json`

### G. Regression tests authored in source

- [x] Cover provider inbox deduplication, payload collision, leases, retry, and
  completion.
- [x] Cover atomic verified normalized receipt before processing claim.
- [x] Cover normalized-fact immutability through direct SQL mutation attempts.
- [x] Cover dead-letter exclusion from automatic retry and explicit operator
  replay.
- [x] Cover recovery from durable normalized facts without provider reparsing.
- [x] Cover permanent dead-lettering of legacy events without normalized facts.
- [x] Cover payment/refund owner lifecycle application and idempotent replay.
- [x] Cover bounded worker configuration in source tests.
- [ ] Execute the authored tests in a dependency-complete local environment.
- [ ] Retain test output as execution evidence instead of source-only evidence.

Evidence:

- `crates/rustok-migrations/tests/payment_provider_event_inbox_smoke.rs`
- `crates/rustok-migrations/tests/payment_provider_event_replay_smoke.rs`
- `crates/rustok-migrations/tests/payment_provider_event_recovery_smoke.rs`
- `crates/rustok-migrations/tests/payment_provider_event_normalized_facts_smoke.rs`
- payment provider/lifecycle tests under `crates/rustok-payment/tests/`

## Verification and promotion checklist

These tasks remain unchecked until they are actually executed. Source inspection is
not sufficient.

### Static contract verification

- [ ] `npm run verify:payment:storefront-boundary`
- [ ] `npm run verify:ecommerce:fba`
- [ ] `npm run verify:ecommerce:provider-spi-evidence`
- [ ] `cargo xtask module validate payment`

### Compile and module tests

- [ ] `cargo check -p rustok-payment --all-features`
- [ ] `cargo check -p rustok-server --features mod-payment`
- [ ] `cargo xtask module test payment`
- [ ] Targeted payment provider operation, inbox, replay, recovery, and lifecycle
  tests.

### Database verification

- [ ] Apply all payment migrations to a clean SQLite database.
- [ ] Roll back and reapply the payment migration chain where down migrations are
  supported.
- [ ] Apply all payment migrations to PostgreSQL.
- [ ] Exercise concurrent claim, expired lease, duplicate delivery, payload
  collision, normalized-fact immutability, and dead-letter replay on PostgreSQL.
- [ ] Verify indexes and query plans for retryable and dead-letter scans with
  production-like row counts.

### Transport and runtime verification

- [ ] Start a `mod-payment` server and prove every declared payment router is
  mounted.
- [ ] Exercise authenticated operator read, recovery, and replay endpoints.
- [ ] Prove the background recovery worker starts only in worker-enabled runtime
  profiles and shuts down through `StopHandle`.
- [ ] Prove two server replicas cannot apply the same inbox event concurrently.
- [ ] Prove process termination after owner apply but before inbox completion is
  recovered without a duplicate lifecycle transition.

### External-provider promotion gate

- [ ] Register a concrete external gateway through the host provider registry.
- [ ] Verify a real provider signature before inbox insertion.
- [ ] Exercise authorize, capture, cancel, refund, duplicate delivery,
  out-of-order capture, retry recovery, and dead-letter replay.
- [ ] Retain provider redelivery and operator-replay evidence.
- [ ] Promote evidence status only after the concrete adapter execution succeeds.

## Immediate execution order

1. [ ] Correct the payment evidence terminology from raw-payload retention to
   SHA-256 hash-only audit semantics.
2. [ ] Run static verifiers and fix source/registry drift.
3. [ ] Run payment and server compile checks.
4. [ ] Run SQLite migrations and targeted regression tests.
5. [ ] Run PostgreSQL migration and concurrency scenarios.
6. [ ] Run mounted HTTP and scheduled-worker recovery scenarios.
7. [ ] Integrate and execute a production-like gateway adapter.
8. [ ] Reassess FFA/FBA promotion only after retained runtime evidence exists.

## Change rules

1. Update this checklist in the same commit as any completed or newly discovered
   payment task.
2. Do not maintain task status in the webhook runbook, contracts, evidence JSON,
   issues, or chat-only/local plans without reflecting it here.
3. Keep payment lifecycle, provider policy, provider-event persistence, and replay
   policy in this module.
4. Never persist or expose raw provider payloads, signatures, SQL messages, or
   provider SDK errors.
5. Keep `rustok-module.toml`, payment/commerce registries, OpenAPI, and this plan
   aligned with every public payment boundary change.
6. Update `docs/modules/registry.md` only when the FFA/FBA boundary status changes.
