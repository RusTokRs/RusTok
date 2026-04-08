# Документация `rustok-workflow`

`rustok-workflow` — модуль визуальной автоматизации на платформенной очереди и
event infrastructure. Он оркестрирует workflow execution поверх событий
платформы и не должен превращаться в второй event bus или transport runtime.

## Назначение

- публиковать канонический workflow runtime contract для triggers, steps и executions;
- держать workflow engine, execution journal и module-owned transport/UI surfaces внутри модуля;
- развивать automation layer поверх platform events без дублирования event transport.

## Зона ответственности

- `WorkflowService`, `WorkflowEngine`, trigger handlers и execution lifecycle;
- workflow storage: definitions, versions, steps, executions и step executions;
- transport surfaces: GraphQL, REST/webhook ingress и module-owned admin UI package;
- step taxonomy (`action`, `emit_event`, `condition`, `delay`, `http`, `alloy_script`, `notify`);
- tenant isolation, RBAC и execution audit для workflow domain.

## Интеграция

- использует platform `EventBus` / `EventTransport` contracts из foundation layer и не владеет transport delivery;
- может использовать `alloy` как capability для отдельных workflow steps без жёсткого registry dependency;
- `apps/server` для workflow остаётся composition root / shim-слоем, а не владельцем transport business logic;
- workflow-generated events публикуются через outbox path, а не через отдельный internal loop.

## Проверка

- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- targeted tests для trigger matching, step execution, tenant isolation и transport/UI contracts

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
