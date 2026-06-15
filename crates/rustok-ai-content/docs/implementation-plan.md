# План реализации `rustok-ai-content`

## Цель

Сделать `rustok-ai-content` owner-слоем для content moderation vertical.

## Этапы

1. Scaffold crate + docs.
2. Перенести `content_moderation` direct wiring.
3. Добавить policy matrix и approval routing integration.

## Execution checkpoint

- Создан начальный scaffold crate и документация.
- Перенесены task/tool identity, generated decision contract и базовая валидация `content_moderation` в `rustok-ai-content`; `rustok-ai` consume-ит validator в direct generation path.
