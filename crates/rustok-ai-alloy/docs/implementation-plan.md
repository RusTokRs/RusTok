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
- Следующий шаг: расширить Alloy-owned script execution policy metadata и добавить targeted tests/evidence при разрешённых проверках.
- Last updated at (UTC): 2026-06-19T06:15:00Z

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `domain_support_adapter`
- Evidence: crate owns Alloy AI vertical task/tool identity, handler adapter API and pure runtime payload validation while executable provider/runtime composition remains in `rustok-ai`.
