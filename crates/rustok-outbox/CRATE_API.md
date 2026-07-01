# rustok-outbox / CRATE_API

## Публичные модули
`entity`, `migration`, `ports`, `relay`, `transactional`, `transport`; `loco` доступен с feature `loco-adapter`.

## Основные публичные типы и сигнатуры
- `pub struct TransactionalEventBus`
- `pub struct OutboxRelay`, `pub struct RelayConfig`, `pub struct RelayMetricsSnapshot`
- `pub struct OutboxTransport`
- `pub struct SysEventsMigration`
- `pub use entity::{Entity as SysEvents, Model as SysEvent}`

## События
- Публикует: `EventEnvelope` в транспорт после фиксации транзакции.
- Потребляет: записи outbox (`sys_events`) для relay/disptach.

## Зависимости от других rustok-крейтов
- `rustok-core`
- `rustok-api`
- `rustok-events`

## Частые ошибки ИИ
- Публикует event напрямую в transport вместо `TransactionalEventBus::publish` внутри tx.
- Путает `OutboxTransport` и реальный L2 transport (`rustok-iggy`).

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
