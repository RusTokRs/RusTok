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
- conversion flows `topic <-> post`, split/merge topic и canonical URL policy, включая запрет cross-target canonical collisions и alias shadowing;
- owner-owned GraphQL query `resolveCanonicalRoute` для canonical URL read contract;
- content-owned GraphQL dataloaders для `nodes`, `node_translations` и `bodies`;
- owner-owned dashboard helper `load_post_stats_snapshot` и DTO `ContentCountSnapshot` для post-статистики без SQL по `nodes` внутри `apps/server`;
- orchestration tables, audit trail и domain events;
- отсутствие product-owned CRUD/runtime adapters для blog/forum/pages.

## Интеграция

- используется `rustok-blog`, `rustok-forum`, `rustok-pages` и `rustok-comments` как shared helper/orchestration contract;
- `rustok-content-orchestration` держит integration bridge и GraphQL mutations conversion path;
- `apps/server` только собирает GraphQL roots, регистрирует owner-owned dataloaders из owner/support crates и композирует content-owned dashboard post analytics helper;
- `rustok-index` зависит от canonical URL и reindex semantics, но не становится владельцем orchestration logic;
- RBAC, idempotency и unsafe-input validation обязаны оставаться частью module-level contract.

## Проверка

- `npm run verify:content:orchestration` — compile-free guardrail для orchestration RBAC/idempotency/audit/outbox/canonical URL invariants, targeted collision rollback/no-outbox evidence markers и синхронизации docs/registry.
- `cargo xtask module validate content`
- `cargo xtask module test content`
- targeted tests для orchestration commands, canonical URL collision/alias shadowing rollback, locale fallback и rich-text validation

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [RT JSON v1](../../../docs/standards/rt-json-v1.md)
