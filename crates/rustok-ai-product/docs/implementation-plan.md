# План реализации `rustok-ai-product`

## Цель

Сделать `rustok-ai-product` канонической domain-owned точкой для product AI verticals.

## Этапы

1. Создать crate + docs contracts.
2. Перенести `product_attributes` direct wiring из `rustok-ai` в registration API этого crate.
3. Перенести `product_copy` direct wiring.
4. Добавить targeted tests и валидацию contracts.

## Execution checkpoint

- Создан начальный scaffold crate и документация.
- Перенесены generated payload contracts и базовая валидация `product_copy` / `product_attributes` в `rustok-ai-product`; `rustok-ai` consume-ит эти validators в direct generation path.
