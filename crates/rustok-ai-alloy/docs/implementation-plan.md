# `rustok-ai-alloy` — Implementation Plan

## Goal

Make `rustok-ai-alloy` the domain-owned adapter crate for Alloy scripting AI verticals, starting with `alloy_code` task/tool identity and runtime payload validation.

## Stages

1. Scaffold crate + docs.
2. Move `alloy_code` task/tool identity from `rustok-ai` to alloy-owned descriptor API.
3. Move validation helpers for runtime payload JSON.
4. Add targeted verification and synchronize central registry evidence.

## Execution checkpoint

- Support crate `rustok-ai-alloy` created with local docs.
- `ALLOY_CODE_TASK_SLUG`, `ALLOY_CODE_TOOL_NAME`, descriptor registry and `register_alloy_ai_vertical_handlers` adapter API moved.
- Canonical runtime payload validation (`runtime_payload_json` must be absent/blank or a JSON object) moved to alloy-owned pure helper, consumed by `rustok-ai` direct alloy runtime.
- Alloy-owned script execution policy metadata (`alloy_script_execution_policy`) with `allowed_operations`, descriptor-level `runtime_operation`/`transport_owner`, registry `contracts/ai-alloy-policy-registry.json`, static evidence `contracts/evidence/ai-alloy-policy-static-matrix.json` and fast verifier `scripts/verify/verify-ai-alloy-policy.mjs` added without compilation.
- Next step: when compilations are allowed, run targeted Rust tests for `validate_runtime_payload`, descriptor policy and `allowed_operations`; until then, source/static evidence lock remains the primary gate.
- Added compile-free static evidence coverage in the unified `scripts/verify/verify-ai-domain-verticals.mjs` gate for descriptor ownership, runtime binding seams, and validation/policy tests without compilation.
- Last updated at (UTC): 2026-06-24T00:00:00Z

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `domain_support_adapter`
- Evidence: crate owns Alloy AI vertical task/tool identity, handler adapter API, pure runtime payload validation, and script execution policy metadata through `alloy_script_execution_policy`, including `allowed_operations` plus descriptor `runtime_operation`/`transport_owner`; registry `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json` and static matrix `crates/rustok-ai-alloy/contracts/evidence/ai-alloy-policy-static-matrix.json` are checked by `scripts/verify/verify-ai-alloy-policy.mjs` while executable provider/runtime composition remains in `rustok-ai`.
