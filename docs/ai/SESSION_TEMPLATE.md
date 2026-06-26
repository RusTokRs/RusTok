---
id: doc://docs/ai/SESSION_TEMPLATE.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Шаблон AI-сессии (RusToK)

Используйте этот шаблон перед генерацией кода в любом модуле.

## Обязательная преамбула для AI

> Перед генерацией кода:
> 1) прочитай `docs/AI_CONTEXT.md`;  
> 2) прочитай `CRATE_API.md` целевого крейта (если файл есть);  
> 3) прочитай `README.md` целевого крейта;  
> 4) если изменения затрагивают Loco/Iggy/MCP/Outbox/Telemetry — сначала сверяйся с reference-пакетом (`docs/references/loco/README.md`, `docs/references/iggy/README.md`, `docs/references/mcp/README.md`, `docs/references/outbox/README.md`, `docs/references/telemetry/README.md`);
> 5) проверь инварианты событий (`publish_in_tx`, `EventEnvelope`, обработчики).

## Мини-шаблон промпта

```text
Контекст:
- Прочитай docs/AI_CONTEXT.md.
- Прочитай CRATE_API.md целевого крейта (если есть).
- Прочитай README.md целевого крейта.
- Если меняешь Loco/Iggy/MCP/Outbox/Telemetry — сначала прочитай соответствующий reference-пакет в `docs/references/`.

Задача:
- <кратко опиши задачу>

Ограничения:
- Не выдумывать API Loco/Iggy/внутренних крейтов.
- Для transactional flow использовать publish_in_tx.
- Проверить соответствие EventEnvelope и обработчиков.

Результат:
- Дай patch по файлам.
- Обнови документацию, если изменены контракты.
- Укажи проверки/тесты, которые выполнил.
```

## Чек-лист перед ответом

- [ ] Использованы только существующие публичные типы/методы.
- [ ] Для Loco/Iggy/MCP/Outbox/Telemetry изменений сначала проверен reference-пакет.
- [ ] Для write + event применён `publish_in_tx` (где требуется).
- [ ] Event handlers совместимы с текущим `EventEnvelope`.
- [ ] Обновлены релевантные docs (`docs/` и локальные docs модуля).
