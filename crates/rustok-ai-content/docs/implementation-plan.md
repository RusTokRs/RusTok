# План реализации `rustok-ai-content`

## Цель

Сделать `rustok-ai-content` owner-слоем для content AI verticals: content moderation и blog draft generated payload contracts.

## Этапы

1. Scaffold crate + docs.
2. Перенести `content_moderation` direct wiring.
3. Перенести `blog_draft` task/tool identity и generated payload validation в content-owned support crate.
4. Добавить policy matrix и approval routing integration.

## Execution checkpoint

- Создан начальный scaffold crate и документация.
- Перенесены task/tool identity, generated decision contract и базовая валидация `content_moderation` в `rustok-ai-content`; `rustok-ai` consume-ит validator в direct generation path.
- Перенесены `blog_draft` task/tool identity, `GeneratedBlogDraft` и blank-field validation в `rustok-ai-content`; direct registry теперь подключает blog handler через content-owned adapter API, а `rustok-ai` остаётся executable runtime composition owner.
