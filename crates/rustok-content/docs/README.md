# rustok-content docs

В этой папке хранится документация модуля `crates/rustok-content`.

## Documents

- [Implementation plan](./implementation-plan.md)

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)


## Orchestration

- `ContentOrchestrationService` реализует кросс-доменные use-case операции: `promote_topic_to_post`, `demote_post_to_topic`, `split_topic`, `merge_topics` с транзакционным переносом reply/comment узлов через node-layer и публикацией доменных событий.
- Для orchestration-операций добавлены RBAC checks, idempotency key (через `content_orchestration_operations`), audit log (`content_orchestration_audit_logs`) и обновление canonical/cross-link метаданных узлов.
- Mapping статусов/метаданных вынесен в `src/services/orchestration_mapping.rs` для переиспользования без дублирования в API handlers.

## Canonical URL policy после конвертаций

### 1. Канонический источник после `topic ↔ post` конвертации

- Каноническим источником всегда считается **текущий активный node** после orchestration-операции.
- При `promote_topic_to_post` canonical-представлением становится `post`-узел (даже если у него сохранена ссылка на исходный topic).
- При `demote_post_to_topic` canonical-представлением становится `topic`-узел.
- Исходная сущность после конвертации трактуется как **legacy alias** и не должна оставаться самостоятельной индексируемой страницей.

### 2. Redirect + canonical tags

- Для всех legacy URL после конвертации обязан отдаваться redirect на canonical URL (предпочтительно `301` для стабильной миграции URL).
- На canonical-странице выставляется `<link rel="canonical">` на сам canonical URL (self-canonical).
- На неканонических представлениях (если они временно доступны по бизнес-причинам) canonical tag должен указывать на canonical URL, а страница помечается `noindex` до полного вывода из обращения.
- Redirect и canonical tag должны указывать на один и тот же целевой URL; расхождение считается ошибкой конфигурации маршрутизации.

### 3. Защита от дубль-страниц

- Нельзя одновременно держать индексируемыми старый `topic` URL и новый `post` URL для одного и того же контента.
- В любой момент времени для одной логической публикации допускается ровно один canonical URL на locale.
- Все alias/исторические URL фиксируются как redirect-источники и исключаются из самостоятельной индексации.

### 4. Правила slug collision

- Уникальность slug обеспечивается в рамках `tenant + locale`.
- Если новый canonical slug конфликтует с активным slug другой сущности, операция конвертации/миграции не завершает publish до разрешения конфликта.
- Разрешение collision выполняется детерминированно: новый slug (или один из конфликтующих) переводится в согласованный fallback-формат (например, с суффиксом), после чего canonical mapping и redirect-цепочка пересчитываются атомарно.
- После разрешения collision старый slug становится alias и должен вести redirect на новый canonical URL.

### 5. Locale-specific slug migration

- Миграция slug выполняется по локалям независимо: canonical URL определяется отдельно для каждой locale-версии.
- Если в locale нет собственного slug/перевода, применяется fallback locale по tenant policy, но canonical URL всё равно должен быть единственным для конкретного resolved locale.
- При появлении локализованного slug позже (после fallback-периода) старый fallback URL становится alias с redirect на новый locale-specific canonical URL.

### 6. Связь с индексатором (`rustok-index`)

- Любая смена canonical URL (конвертация, collision-resolution, locale-migration) обязана порождать:
  1. событие на **reindex** canonical-цели;
  2. событие на **purge/deindex** устаревших URL aliases.
- Порядок обязателен для at-least-once delivery: сначала фиксируется новый canonical mapping, затем публикуются события reindex/purge в рамках одного надежного event-flow (через outbox).
- Обработчики индексатора должны быть идемпотентны: повторная доставка не должна повторно «оживлять» устаревший URL и не должна приводить к дублирующим документам в read-model/search index.
