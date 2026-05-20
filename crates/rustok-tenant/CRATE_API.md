# rustok-tenant / CRATE_API

## Публичные модули
`dto`, `entities`, `error`, `services`.

## Основные публичные типы и сигнатуры
- `pub struct TenantModule`
- Публичные tenant DTO/сервисы из `services`.
- `TenantModule` реализует `RusToKModule` с `ModuleKind::Core`.

## События
- Публикует: `tenant.created`, `tenant.updated`, `tenant.module.toggled` (через `TransactionalEventBus`, если он передан в `TenantService::with_event_bus`).
- Потребляет: N/A.

## Зависимости от других rustok-крейтов
- `rustok-core`
- `rustok-events`
- `rustok-outbox`

## Частые ошибки ИИ
- Смешивает `tenant slug` и внутренний `tenant_id`.
- Не добавляет tenant isolation в запросы и проверки доступа.

## Минимальный набор контрактов

### Входные DTO/команды
- Входной контракт формируется публичными DTO/командами из crate (см. разделы с `Create*Input`/`Update*Input`/query/filter выше и соответствующие `pub`-экспорты в `src/lib.rs`).
- Все изменения публичных полей DTO считаются breaking-change и требуют синхронного обновления transport-адаптеров `apps/server`.

### Доменные инварианты
- Инварианты модуля фиксируются в сервисах/стейт-машинах и валидации DTO; недопустимые переходы/параметры должны завершаться доменной ошибкой.
- Инварианты multi-tenant boundary (tenant/resource isolation, auth context) считаются обязательной частью контракта.

### События / outbox-побочные эффекты
- Если модуль публикует доменные события, публикация должна идти через транзакционный outbox/transport-контракт без локальных обходов.
- Формат event payload и event-type должен оставаться обратно-совместимым для межмодульных потребителей.

### Ошибки / коды отказов
- Публичные `*Error`/`*Result` типы модуля определяют контракт отказов и не должны терять семантику при маппинге в HTTP/GraphQL/CLI.
- Для validation/auth/conflict/not-found сценариев должен сохраняться устойчивый error-class, используемый тестами и адаптерами.
