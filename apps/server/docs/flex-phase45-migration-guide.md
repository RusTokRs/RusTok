# Flex Phase 4.5 Migration Guide (apps/server)

## Контекст

В рамках Phase 4.5 Flex attached-mode выносится из `apps/server` в agnostic crate `crates/flex`.

Цель изменений в server-слое:
- оставить только transport/adapters и bootstrap wiring;
- убрать дублирование контрактов между `apps/server` и `crates/flex`.

## Что изменилось

### 1) Registry contracts импортируются напрямую из `flex`

Ранее `apps/server` использовал временный compatibility re-export модуль:
- `apps/server/src/services/field_definition_registry.rs`.

Теперь прямой путь:
- `use flex::{FieldDefRegistry, FieldDefinitionService, FieldDefinitionView, ...};`

Области применения:
- GraphQL Flex (`query`, `mutation`, `types`)
- Bootstrap registry (`field_definition_registry_bootstrap`)
- Field definition cache service

### 2) Error mapping вынесен в agnostic слой

- Доменный mapping живёт в `flex::map_flex_error()` (`crates/flex/src/errors.rs`)
- В GraphQL остаётся только адаптация transport-ошибки в `FieldError`.

### 3) Cache hooks вынесены в orchestration слой `flex`

Используются helper-ы:
- `flex::list_field_definitions_with_cache(...)`
- `flex::invalidate_field_definition_cache(...)`

Server-кеш реализует порт:
- `impl flex::FieldDefinitionCachePort for FieldDefinitionCache`

## Правила для нового кода в `apps/server`

1. Не добавлять новые типы/трейты registry для Flex в `apps/server`.
2. Для generic контрактов attached-mode использовать только API crate `flex`.
3. Для GraphQL/REST-слоя оставлять только transport concerns (auth, RBAC gate, input/output mapping, error adaptation).
4. Для кеширования использовать `FieldDefinitionCachePort` + orchestration helper-ы из `flex`.

## Проверочный чеклист после миграции

- [ ] В `apps/server/src` нет импортов из `services::field_definition_registry`
- [ ] GraphQL Flex проходит compile check
- [ ] Bootstrap registry использует `flex::FieldDefinitionService`
- [ ] Документация `crates/flex/docs/implementation-plan.md` синхронизирована

## Оставшиеся долги

По состоянию Phase 4.5 остаётся:
- устранить оставшееся дублирование между server adapters и agnostic-модулем (пункт в implementation plan);
- при необходимости выделить дополнительные transport-agnostic helper-ы из server adapters в `crates/flex`.
