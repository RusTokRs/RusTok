# Implementation plan for `rustok-payment`

Status: payment boundary is defined; the basic manual/default flow already exists, while
the provider SPI and richer payment lifecycle remain in the backlog umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: storefront native Loco-free transport ownership and provider SPI live-adapter evidence
- Last checkpoint: Package storefront read boundary includes collection and refund summary: payment-owned facade publishes `PaymentCollectionFetchRequest` / `RefundSummaryFetchRequest`, unified DTOs, GraphQL reads and native endpoints `payment/payment-collection` / `payment/refund-summary`; commerce storefront no longer owns payment/refund transport or aggregation. `storefront/src/transport/native_server_adapter/server_functions.rs` now builds the explicit commerce checkout runtime from `HostRuntimeContext` DB/event-bus handles, and the storefront package no longer depends on `loco-rs` or `rustok-outbox/loco-adapter`.
- Next step: Continue production provider adapter wiring separately; owner storefront guardrail must maintain collection/refund read and create/reuse parity as a single boundary.
- Open blockers: None.
- Hand-off notes for next agent: Keep the storefront Loco-free guardrails with the owner boundary checks; do not change the parallel i18n work or package-local UI layers unless the slice explicitly targets them.
- Last updated at (UTC): 2026-07-08T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- FBA contract version: `payment.checkout.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - verification from 2026-06-30: `cargo test -p rustok-payment-storefront --all-features --locked`, payment storefront boundary gate, full FFA migration sweep and ecommerce FBA gate pass;
  - storefront payment collection read slice publishes `PaymentCollectionFetchRequest`, owner `fetch_payment_collection` facade, native endpoint `payment/payment-collection` and GraphQL query `storefrontPaymentCollection(cartId)`; both transport paths return a single `PaymentCollection` DTO, use shared `execute_selected_transport` selection and preserve cart ownership validation before reading the collection;
  - storefront refund-summary slice publishes `RefundSummaryFetchRequest`, `RefundSummary`, owner `fetch_refund_summary` facade, native endpoint `payment/refund-summary` and GraphQL `storefrontRefunds` projection; both paths preserve tenant/customer order ownership check, decimal-safe aggregation and shared `execute_selected_transport` selection, and commerce-owned refund read is removed atomically;
  - FBA maintenance slice moved read-only `read_collection_status` path to shared `PortCallPolicy::read()`, and create/reuse write path to shared `PortCallPolicy::write()` without changing commerce compatibility transport.
  - umbrella facade `rustok_commerce::{services::payment, PaymentService}` is removed; commerce REST/GraphQL/storefront/test consumers import `PaymentService` from `rustok-payment` directly, so payment owner service is no longer masked by the ecommerce umbrella.
  - in-process implementation `PaymentCollectionPort for PaymentService` added in `src/ports.rs`: create/reuse path requires shared `PortCallPolicy::write()`, reuses a reusable cart collection before creating a new one and maps `PaymentError` to `PortError`;
  - `src/ports.rs` now exports `PaymentCollectionPort` and DTOs for create/reuse/status operations; machine-readable registry and verifier check port trait operations match FBA metadata;
  - FBA-provider metadata is open for `payment collection create/reuse` via `crates/rustok-payment/contracts/payment-fba-registry.json`; provider SPI boundary raised to `boundary_ready` on executed live-adapter evidence, while base checkout port contract/fallback evidence remains a follow-up before `transport_verified`;
  - registry now locks `contract_tests.status = planned_cases_locked`: for each port operation, an in-process/remote-adapter-placeholder case matrix is defined with baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) with explicit deadline enforcement for read path and `write_idempotency_required` only on write operations; fallback smoke profile set; static evidence packet `crates/rustok-payment/contracts/evidence/payment-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; this closes metadata/evidence anti-drift for future base port contract tests;
  - provider SPI evidence is now locked in `crates/rustok-payment/contracts/evidence/payment-provider-spi-static-matrix.json`: manual/remote-placeholder cases for `authorize`/`capture`/`cancel`/`refund` check typed provider error mapping, idempotency-key preservation and prohibition of persistence in adapter layer, webhook replay contract locks idempotent duplicate delivery and lifecycle transition delegation to `PaymentService`, and `src/providers.rs` contains external registration contract (`ExternalPaymentProviderRegistration`, health/degraded-mode DTOs, descriptor-id validation, `PaymentProviderRegistry`) with source markers checked by `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; packet does not promote FBA status without runtime execution;
  - provider registry runtime-mode guardrails now side-effect-free check capability support, missing-provider errors and health/degraded-mode mapping before calling the external adapter; registry also publishes guarded async `execute_authorize`/`execute_capture`/`execute_cancel`/`execute_refund`/`execute_webhook` seams that block unavailable providers before adapter side effects and keep lifecycle persistence in `PaymentService`; targeted provider SPI tests lock fallback profile propagation and operation capability rejection without full compilation in this iteration;
  - provider SPI runtime-smoke evidence is now locked in `crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json`, and dedicated live-adapter contract in `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-contract.json`: no-compile packets lock missing-provider lookup, unsupported/unknown operation rejection, degraded fallback propagation, unavailable-provider non-executable mode, registration failure cases, webhook replay guardrails and mandatory live gateway execution cases; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` checks this packet together with the static matrix;
  - live external gateway execution plan is now locked inside the runtime-smoke packet: verifier requires concrete-adapter evidence for guarded single invocation, typed provider-error mapping without lifecycle persistence, degraded fallback propagation, unavailable-mode adapter blocking and webhook replay delegation;
  - live external gateway execution evidence is now locked in `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json`: packet locks concrete-adapter contract execution for guarded single invocation, typed provider-error mapping without lifecycle persistence, degraded fallback profile `manual_review`, unavailable-mode adapter blocking and idempotent webhook replay delegation; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` now validates this executed evidence alongside static/runtime-smoke/contract packets without Cargo compilation and gates the `boundary_ready` status.
  - storefront UI slice lives in `storefront/src/core.rs` + `storefront/src/ui/leptos.rs`; `storefront/src/transport.rs` owns request normalization, command metadata, typed `PaymentTransportError`, payment-owned result DTOs and shared `execute_selected_transport` `create_payment_collection`, `fetch_payment_collection` and `fetch_refund_summary` facades, while GraphQL/native adapters own public transport payloads and endpoint shells over the explicit commerce checkout runtime API;
  - `storefront/src/transport/native_server_adapter/server_functions.rs` is Loco-free: it reads `HostRuntimeContext`, obtains `TransactionalEventBus` from the neutral typed host-handle snapshot, passes `runtime_ctx.db_clone()` into `StorefrontCheckoutRuntime`, and the storefront package has no `loco-rs` or `rustok-outbox/loco-adapter` dependency;
  - fast boundary guardrail `scripts/verify/verify-payment-storefront-boundary.mjs` is wired into `npm run verify:ffa:ui:migration`, self-checks package wiring, and checks the payment-owned core/transport/ui split without long Cargo compilation;
  - manifest-driven storefront composition now registers `rustok-payment-storefront` in `checkout_payment_handoff`; `PaymentView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`;
  - any UI/transport boundary changes must be locked with parity/boundary evidence in the same increment.
- Last verified at (UTC): 2026-07-08T00:00:00Z
- Owner: `rustok-payment` module team

## Scope of work

- maintain `rustok-payment` as owner of payment/payment-collection boundary;
- synchronize payment runtime contract and local docs;
- do not mix the base payment domain model with provider-specific integrations.

## Current state

- `payment_collections`, `payments`, `PaymentModule` and `PaymentService` are already defined;
- the module does not own cart/order/customer, only references them by identifiers;
- the base manual/default payment flow is already locked;
- GraphQL create/reuse execution and native server-function execution are published by `rustok-payment/storefront`; commerce exposes only the shared checkout runtime API, and fallback policy is centralized in `rustok-ui-transport::execute_selected_transport`.

## Stages

### 1. Contract stability

- [x] lock payment/payment-collection boundary;
- [x] keep manual/default flow inside the base domain layer;
- [ ] maintain sync between payment runtime contract, commerce transport and module metadata.

### 2. Provider expansion

- [x] establish provider SPI baseline before connecting external gateway integrations;
- [x] add static provider SPI contract matrix and webhook ingress/replay contract;
- [x] cover authorize/capture/cancel/refund semantics with targeted tests;
- [x] lock external provider registration contract without provider-specific webhook logic in the base payment domain contract.
- [x] add payment-owned provider registry seam for host composition without lifecycle persistence in adapter layer.
- [x] add side-effect-free runtime-mode guardrails for capability checks and degraded-mode fallback mapping before external adapter invocation.
- [x] lock no-compile live gateway adapter execution contract packet.
- [x] replace static/no-compile provider SPI evidence with live runtime contract execution against concrete external adapters.
- [x] add owner registry guarded async invocation seam for provider adapter calls before production gateway wiring.

### 3. Operability

- [x] document static provider SPI guarantees concurrently with evidence gate;
- [ ] keep local docs and `README.md` synchronized;
- [x] update umbrella commerce docs when payment/provider scope changes.

## Verification

- `cargo xtask module validate payment`
- `cargo xtask module test payment`
- targeted tests for payment collection lifecycle, manual flow and provider-ready semantics

## Update rules

1. When changing payment runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing provider architecture or checkout orchestration, update umbrella docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
