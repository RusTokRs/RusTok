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
