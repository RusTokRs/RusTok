# rustok-core / CRATE_API

## Публичные модули
`async_utils`, `cache`, `config`, `content_format`, `context`, `error`, `events`, `field_schema`, `grapesjs`, `health`, `i18n`, `id`, `locale`, `metrics`, `migrations`, `module`, `permissions`, `rbac`, `registry`, `resilience`, `rt_json`, `security`, `state_machine`, `tenant_validation`, `tracing`, `typed_error`, `types`, `utils`.

## Основные публичные типы и сигнатуры
- `pub trait RusToKModule` — базовый контракт модуля платформы.
- `pub struct AppContext` — общий runtime-контекст приложения.
- `pub enum DomainEvent`, `pub struct EventEnvelope` — события домена и обёртка для транспорта.
- `pub trait EventTransport` — транспорт событий.
- `pub enum Error`, `pub type Result<T>` — unified error model.
- `pub struct ModuleRegistry` — реестр модулей и зависимостей.
- `pub enum UserRole`, `pub enum UserStatus` — shared identity primitives.
- `pub struct CustomFieldsSchema`, `pub struct FieldDefinition` — flex/custom-fields contract.
- `pub fn generate_id()` — canonical ID generation.

## События
- Публикует: базовые доменные события через `DomainEvent` (определяет контракт, не бизнес-эмиттер).
- Потребляет: N/A (инфраструктурный контрактный слой).

## Зависимости от других rustok-крейтов
- `rustok-telemetry`
- `rustok-outbox`

## Частые ошибки ИИ
- Путает `AppContext` из `rustok_core::context` с локальными контекстами сервисов.
- Импортирует `DomainEvent` из старых путей вместо `rustok_core`/`rustok-events`.
- Считает `rustok-core` доменным модулем (`RusToKModule`) — это инфраструктурный core.

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
