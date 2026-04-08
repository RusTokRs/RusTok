# Документация `rustok-content`

`rustok-content` — shared content/orchestration модуль платформы. Он больше не
владеет product CRUD для blog/forum/pages, а держит общие rich-text, locale и
conversion contracts, на которые опираются доменные модули.

## Назначение

- публиковать shared content/orchestration runtime contract;
- удерживать locale normalization, rich-text validation и conversion semantics внутри модуля;
- давать доменным модулям стабильный orchestration layer без возврата к shared product storage.

## Зона ответственности

- `ContentOrchestrationService`, orchestration audit/idempotency и canonical URL state;
- shared rich-text и locale fallback helpers;
- conversion flows `topic <-> post`, split/merge topic и canonical URL policy;
- orchestration tables, audit trail и domain events;
- отсутствие product-owned CRUD/runtime adapters для blog/forum/pages.

## Интеграция

- используется `rustok-blog`, `rustok-forum`, `rustok-pages` и `rustok-comments` как shared helper/orchestration contract;
- `apps/server` держит только integration bridge и composition root вокруг orchestration path;
- `rustok-index` зависит от canonical URL и reindex semantics, но не становится владельцем orchestration logic;
- RBAC, idempotency и unsafe-input validation обязаны оставаться частью module-level contract.

## Проверка

- `cargo xtask module validate content`
- `cargo xtask module test content`
- targeted tests для orchestration commands, canonical URL flows, locale fallback и rich-text validation

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [RT JSON v1](../../../docs/standards/rt-json-v1.md)
