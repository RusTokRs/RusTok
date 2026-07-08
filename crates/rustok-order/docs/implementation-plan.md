# Implementation plan for `rustok-order`

Status: order boundary is separated; the module owns order write-side lifecycle,
outbox publication and module-owned admin UI, while post-order and transport parity
are collected by umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: storefront_native_loco_free
- Last checkpoint: `OrderStatsSnapshot` and `load_order_stats_snapshot` moved to `rustok-order`; `apps/server::RootQuery::dashboard_stats` only composes the owner helper behind feature `mod-order` and no longer contains SQL for `order.placed` events. The boundary is locked by `apps/server/tests/module_surface_boundary_guard.rs` without compilation. Storefront complete-checkout native server function now consumes `HostRuntimeContext` plus a typed `TransactionalEventBus` host handle, no longer imports Loco `AppContext`, and no longer depends on `loco-rs` or the outbox `loco-adapter` feature.
- Next step: maintain parity of the public GraphQL order contract while post-order surfaces continue moving to owner admin/storefront packages; continue removing remaining module-specific server GraphQL artifacts in small no-compile slices.
- Open blockers: server OpenAPI contract test under default features previously ran into existing compile errors outside order/commerce (`rustok-pages-admin`, server build service/module lifecycle/graphql mutations); targeted order lifecycle and `rustok-commerce` check remain the main gate for this slice.
- Hand-off notes for next agent: After each returns/refund/exchange/claim increment, update FFA evidence and FBA placeholder, README/admin docs and central registry in the same PR.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- FBA contract version: `order.checkout_completion.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - Boundary readiness update: `crates/rustok-order/contracts/order-fba-registry.json` now has `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`; `npm run verify:ecommerce:fba` gates the owner `OrderService` checkout-completion path, while remote/base fallback smoke remains a follow-up before `transport_verified`.
  - umbrella facade `rustok_commerce::{services::order, OrderService}` is removed; commerce REST/GraphQL/admin/storefront/test consumers import `OrderService` from `rustok-order` directly, so order owner service is no longer masked by the ecommerce umbrella.
  - FBA provider registry `crates/rustok-order/contracts/order-fba-registry.json` now also declares `ai-order` as an operator-context consumer of `CheckoutCompletionPort` / `order.checkout_completion.v1` `read_order_status`, with `generate_summary_without_live_status`, `require_operator_review`, and `skip_prefill_execution` degraded modes locked by `scripts/verify/verify-ai-fba-baseline.mjs`.
  - FBA maintenance slice moved the read-only checkout result/status paths to shared `PortCallPolicy::read()`, and the complete-checkout write path to shared `PortCallPolicy::write()` without changing the temporary commerce transport handoff.
  - `src/ports.rs` now exports `CheckoutCompletionPort` and DTO for complete/result/status operations; machine-readable registry and verifier check port trait operations match FBA metadata;
  - FBA-provider metadata is open for `checkout completion/result` through `crates/rustok-order/contracts/order-fba-registry.json`; status remains `in_progress` until contract tests/remote transport evidence appear, which would allow promotion above embedded checkout compatibility;
  - registry now locks `contract_tests.status = planned_cases_locked`: for each port operation, an in-process/remote-adapter-placeholder case matrix is defined, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) and corrected `write_idempotency_required` only for `complete_checkout`; read-only result/status cases no longer require write idempotency; fallback smoke profile set; static evidence packet `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; this closes metadata/evidence anti-drift for future contract tests, but does not raise status without runtime evidence;
  - `in_process_provider_impl` now locks `OrderService` as owner implementation for `CheckoutCompletionPort`: write-path calls `PortCallPolicy::write()` before owner `create_order_with_channel`, confirms order lifecycle through `confirm_order` and reloads locale-aware snapshot when locale context is present; read status calls `PortCallPolicy::read()` before owner `get_order`, and cart-id result projection remains a typed unavailable gap until storage projection appears; fast verifier checks these semantics without full compilation;
  - any UI/transport boundary changes must be locked with parity/boundary evidence in the same increment;
  - manifest-driven storefront composition now registers `rustok-order-storefront` in `checkout_result_handoff`; `OrderView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`;
  - storefront native checkout completion is now owner-owned and Loco-free: `storefront/src/transport/native_server_adapter/server_functions.rs` publishes `order/complete-checkout` over the explicit `rustok_commerce::storefront_checkout_runtime` API using `HostRuntimeContext` DB/event-bus handles, so commerce no longer keeps the native order owner-operation wrapper and the storefront package no longer depends on `loco-rs` or `rustok-outbox/loco-adapter`;
  - dashboard order analytics are now owner-owned: `rustok-order::load_order_stats_snapshot` reads `order.placed` outbox events, and `apps/server::RootQuery` only composes the result and is checked by the boundary guard without compilation;
  - admin FFA slice added framework-agnostic `admin/src/core/` list/filter request policy, module-owned `admin/src/transport/mod.rs` facade and explicit Leptos render adapter `admin/src/ui/leptos.rs`, locked by `scripts/verify/verify-order-admin-boundary.mjs`; storefront owns `CompleteCheckoutRequest`, `CheckoutAdjustment`, `CheckoutCompletion`, the MissingServer-gated `complete_checkout` facade, `storefront/src/transport/graphql_adapter.rs` with the complete-checkout GraphQL mutation/mapping and `storefront/src/transport/native_server_adapter/server_functions.rs` with the Loco-free `order/complete-checkout` server-function shell over the explicit commerce checkout runtime API; commerce no longer duplicates order GraphQL payload, response projection or native owner-operation wrapper; `scripts/verify/verify-order-storefront-boundary.mjs` and `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` lock the owner boundary.
- Last verified at (UTC): 2026-07-08T00:00:00Z
- Owner: `rustok-order` module team

## Scope of work

- keep `rustok-order` as the owner order lifecycle and order snapshots;
- synchronize order runtime contract, event flow, admin UI and local docs;
- do not mix order write model with payment/fulfillment/provider orchestration.

## Current state

- `orders` and `order_line_items` are already module-owned;
- `order_adjustments` are already module-owned and lock language-neutral promotion/discount snapshot without display labels;
- `order_tax_lines` now also carry typed `provider_id`, and checkout transfers provider-aware tax snapshot
  from cart without metadata-only fallback;
- write-side lifecycle and order events are already locked inside the module;
- product/variant relationships are stored as snapshot references, without cross-module FK;
- complete-checkout GraphQL execution, native server-function execution, result DTOs and fallback policy are order-owned; commerce exposes only the shared checkout runtime API for orchestration;
- dashboard order analytics (`OrderStatsSnapshot`, `load_order_stats_snapshot`) are already module-owned; server GraphQL does not contain SQL over `order.placed`;
- `rustok-order/admin` publishes module-owned route for order list/detail/lifecycle with `admin/src/core/` request defaults, `admin/src/transport/mod.rs` facade and explicit `admin/src/ui/leptos.rs` render adapter.

## Stages

### 1. Contract stability

- [x] lock order-owned lifecycle and snapshot model;
- [x] add typed order adjustment snapshot with `subtotal_amount`, `adjustment_total` and net `total_amount`;
- [x] keep event publication part of module boundary;
- [x] move admin order UI to module-owned package `rustok-order/admin`;
- [ ] maintain sync between order runtime contract, commerce transport and module metadata.

### 2. Post-order expansion

- [~] evolve returns, refunds, exchanges, claims and order changes as a separate next layer; (started: `order_returns` + `order_return_items` storage, item validation, `OrderService::{create_return,get_return,list_returns,complete_return,cancel_return}` foundation and resolution references of completed return for refund/exchange/claim/order-change orchestration)
- [x] cover lifecycle transitions and failure semantics with targeted tests; (return lifecycle `pending -> completed|cancelled`, second-transition guard, tenant-scoped show)
- [~] maintain compatibility with payment/fulfillment orchestration without blurring order ownership. (started: `order_changes` skeleton holds preview/apply/cancel state without payment/fulfillment side effects)

### 3. Operability

- [~] document new order guarantees simultaneously with changing runtime surface; (returns lifecycle, item-level lines, resolution references of completed return and order-change skeleton checkpoint locked)
- [ ] keep local docs and `README.md` synchronized;
- [ ] update umbrella commerce docs when order/post-order scope changes.

## Verification

- `cargo xtask module validate order`
- `cargo xtask module test order`
- targeted tests for order lifecycle, typed adjustments, outbox events and snapshot invariants

## Update rules

1. When changing order runtime contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md`, `admin/README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing order/payment/fulfillment orchestration, update umbrella docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
