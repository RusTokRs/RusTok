# План реализации `rustok-ai-order`

## Цель

Перенести order AI vertical wiring в domain-owned crate.

## Этапы

1. Scaffold crate + docs.
2. Перенести `order_analytics` direct wiring.
3. Перенести `order_ops_assistant` direct wiring.
4. Добавить targeted verification.

## Execution checkpoint

- Создан начальный scaffold crate и документация.
- Добавлен domain-owned registration metadata API (`order_ai_verticals`) для `order_analytics` / `order_ops_assistant`; runtime handler registration в `rustok-ai` использует эти task/tool constants.
- Ужесточена domain-owned валидация generated payload contracts: `order_analytics` отвергает blank items в строковых массивах, а `order_ops_assistant` отвергает null `prefill`.
- Last updated at (UTC): 2026-06-19T06:15:00Z
- Added compile-free static verification gate `scripts/verify/verify-ai-domain-verticals.mjs` for order descriptors, runtime binding seam, generated payload validators, and sensitive ops-assistant metadata.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - `admin/src/core.rs`, `admin/src/transport.rs`, and `admin/src/ui/leptos.rs` provide the module-owned admin FFA split.
  - Transport exposes a native-server plus GraphQL fallback placeholder profile; concrete host rendering remains a follow-up.
  - FBA support-consumer metadata is locked in `crates/rustok-ai-order/contracts/ai-order-fba-registry.json` for the `CheckoutCompletionPort` / `order.checkout_completion.v1` `read_order_status` dependency and order analytics/ops assistant generated-payload validation, including `generate_summary_without_live_status`, `require_operator_review`, and `skip_prefill_execution` degraded modes, mirrored by `crates/rustok-ai-order/contracts/evidence/ai-order-consumer-static-matrix.json` and source-smoke `crates/rustok-ai-order/contracts/evidence/ai-order-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-fba-baseline.mjs`.
  - The global readiness board uses the canonical hyphenated module slug `ai-order`.
