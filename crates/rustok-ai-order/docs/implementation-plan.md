# `rustok-ai-order` — Implementation Plan

## Goal

Move order AI vertical wiring to a domain-owned crate.

## Stages

1. Scaffold crate + docs.
2. Move `order_analytics` direct wiring.
3. Move `order_ops_assistant` direct wiring.
4. Add targeted verification.

## Execution checkpoint

- Initial scaffold crate and documentation created.
- Domain-owned registration metadata API (`order_ai_verticals`) added for `order_analytics` / `order_ops_assistant`; runtime handler registration in `rustok-ai` uses these task/tool constants.
- Domain-owned generated payload contract validation tightened: `order_analytics` rejects blank items in string arrays, and `order_ops_assistant` rejects null `prefill`.
- Last updated at (UTC): 2026-06-19T06:15:00Z
- Added compile-free static verification gate `scripts/verify/verify-ai-domain-verticals.mjs` for order descriptors, runtime binding seam, generated payload validators, and sensitive ops-assistant metadata.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - `admin/src/core.rs`, `admin/src/transport.rs`, and `admin/src/ui/leptos.rs` provide the module-owned admin FFA split.
  - Transport exposes a build-profile-selected native-server plus GraphQL selected-path profile; concrete host rendering remains a follow-up.
  - FBA support-consumer metadata is locked in `crates/rustok-ai-order/contracts/ai-order-fba-registry.json` for the `CheckoutCompletionPort` / `order.checkout_completion.v1` `read_order_status` dependency and order analytics/ops assistant generated-payload validation, including `generate_summary_without_live_status`, `require_operator_review`, and `skip_prefill_execution` degraded modes, mirrored by `crates/rustok-ai-order/contracts/evidence/ai-order-consumer-static-matrix.json` and source-smoke `crates/rustok-ai-order/contracts/evidence/ai-order-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-fba-baseline.mjs`.
  - Boundary readiness is backed by executable `cargo test -p rustok-ai-order --lib` coverage for order-owned descriptors, sensitive ops assistant metadata and generated payload validation.
  - The global readiness board uses the canonical hyphenated module slug `ai-order`.
