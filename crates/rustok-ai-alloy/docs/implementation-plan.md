# План реализации `rustok-ai-alloy`

## Цель

Сделать `rustok-ai-alloy` domain-owned adapter crate для Alloy scripting AI verticals, начиная с `alloy_code` task/tool identity и runtime payload validation.

## Этапы

1. Scaffold crate + docs.
2. Перенести `alloy_code` task/tool identity из `rustok-ai` в alloy-owned descriptor API.
3. Перенести validation helpers для runtime payload JSON.
4. Добавить targeted verification и синхронизировать central registry evidence.

## Execution checkpoint

- Создан support crate `rustok-ai-alloy` с local docs.
- Перенесены `ALLOY_CODE_TASK_SLUG`, `ALLOY_CODE_TOOL_NAME`, descriptor registry и `register_alloy_ai_vertical_handlers` adapter API.
- Перенесена canonical runtime payload validation (`runtime_payload_json` must be absent/blank or a JSON object) в alloy-owned pure helper, consumed by `rustok-ai` direct alloy runtime.
- Добавлены Alloy-owned script execution policy metadata (`alloy_script_execution_policy`) с `allowed_operations`, descriptor-level `runtime_operation`/`transport_owner`, registry `contracts/ai-alloy-policy-registry.json`, static evidence `contracts/evidence/ai-alloy-policy-static-matrix.json` и быстрый verifier `scripts/verify/verify-ai-alloy-policy.mjs` без компиляции.
- Следующий шаг: при разрешённых компиляциях прогнать targeted Rust tests для `validate_runtime_payload`, descriptor policy и `allowed_operations`; до этого source/static evidence lock остаётся основным gate.
- Added compile-free static evidence coverage in the unified `scripts/verify/verify-ai-domain-verticals.mjs` gate for descriptor ownership, runtime binding seams, and validation/policy tests without compilation.
- Last updated at (UTC): 2026-06-24T00:00:00Z

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `domain_support_adapter`
- Evidence: crate owns Alloy AI vertical task/tool identity, handler adapter API, pure runtime payload validation, and script execution policy metadata through `alloy_script_execution_policy`, including `allowed_operations` plus descriptor `runtime_operation`/`transport_owner`; registry `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json` and static matrix `crates/rustok-ai-alloy/contracts/evidence/ai-alloy-policy-static-matrix.json` are checked by `scripts/verify/verify-ai-alloy-policy.mjs` while executable provider/runtime composition remains in `rustok-ai`.
