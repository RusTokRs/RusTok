# rustok-iggy / CRATE_API

## Публичные модули
`config`, `consumer`, `dlq`, `health`, `partitioning`, `producer`, `replay`, `serialization`, `topology`, `transport`.

## Основные публичные типы и сигнатуры
- `pub struct IggyTransport` (реализация `EventTransport`)
- `pub trait EventSerializer` + `JsonSerializer`, `PostcardSerializer` (serialize/deserialize)
- `pub struct TopologyManager`, `ConsumerGroupManager`, `ConsumedEvent`, `DlqManager`, `ReplayManager`
- `pub fn health_check(...) -> HealthCheckResult`

## События
- Публикует: сериализованные `EventEnvelope` в Iggy stream/topics.
- Потребляет: сообщения из Iggy consumer groups, включая replay/DLQ pipeline.

## Зависимости от других rustok-крейтов
- `rustok-core`
- `rustok-iggy-connector`

## Частые ошибки ИИ
- Пропускает partition key и ломает порядок обработки.
- Использует не тот сериализатор между producer/consumer.

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
