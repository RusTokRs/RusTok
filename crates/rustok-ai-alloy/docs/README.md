# rustok-ai-alloy

Адаптер для AI-вертикалей Alloy.
Владеет дескрипторами и валидацией runtime payload для выполнения Alloy-скриптов (`alloy_code`).

## Контрактная поверхность

- `ALLOY_CODE_TASK_SLUG` / `ALLOY_CODE_TOOL_NAME` фиксируют публичную identity для Alloy Assist.
- `AlloyAiVerticalDescriptor` фиксирует `runtime_operation = run_script`, `runtime_payload_json_shape = absent_blank_or_json_object` и `transport_owner = rustok-ai`.
- `alloy_script_execution_policy()` фиксирует Alloy runtime ownership, `allowed_operations` (`list_scripts`, `get_script`, `validate_script`, `run_script`) и статус remote transport `not_started`.
- `validate_runtime_payload()` разрешает отсутствующий/пустой payload или JSON object и отклоняет массивы/скаляры/невалидный JSON.

План и evidence: [`docs/implementation-plan.md`](./implementation-plan.md), [`contracts/ai-alloy-policy-registry.json`](../contracts/ai-alloy-policy-registry.json).
