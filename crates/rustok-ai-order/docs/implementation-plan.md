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
