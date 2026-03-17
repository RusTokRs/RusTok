# rustok-content / CRATE_API

## Публичные модули
`dto`, `entities`, `error`, `services`, `state_machine`.

## Основные публичные типы и сигнатуры
- `pub struct ContentModule`
- `pub struct NodeService`
- `pub struct Node`, `pub struct NodeTranslation`, `pub struct Body`
- `pub struct ContentNode<S>` + состояния `Draft`, `Published`, `Archived`
- `pub type ContentResult<T>`, `pub enum ContentError`
- `pub fn validate_status_transition(current: &ContentStatus, target: &ContentStatus) -> Result<(), InvalidStatusTransition>`
- `pub struct InvalidStatusTransition { from: ContentStatus, to: ContentStatus }`

## События
- Публикует доменные события контента через `TransactionalEventBus` (создание/обновление/публикация/архивация node).
- Потребляет: внешние доменные события явно не подписывает (бизнес-операции вызываются сервисами).

## Зависимости от других rustok-крейтов
- `rustok-core`
- `rustok-outbox`
- (dev) `rustok-test-utils`

## Частые ошибки ИИ
- Нарушает state-machine прямым изменением статуса — теперь `transition_status_in_tx` выбросит `Validation` на недопустимый переход.
- Путает `entities::Model` SeaORM и DTO ответа API.
- Пропускает `tenant_id` в фильтрах запросов.
- Передаёт глубоко вложенный JSON в поле `metadata` — максимум 5 уровней вложенности.

## Публичный контракт ошибок
- `ContentError::DuplicateSlug { slug, locale }` — конфликт уникальности slug в пределах `tenant_id + locale`.
- `ContentError::ConcurrentModification { expected, actual }` — optimistic locking при `UpdateNodeInput.expected_version`.
- `ContentError::Validation(String)` — в том числе: недопустимый переход статусов (напр. `Draft → Archived`), превышение глубины вложенности `metadata` (> 5 уровней).
- Первые два варианта конвертируются в `RichError` с `ErrorKind::Conflict` и кодами `DUPLICATE_SLUG` / `CONCURRENT_MODIFICATION`.

## State machine — допустимые переходы статусов
Единственным источником правды является `validate_status_transition` из `state_machine.rs`:

| Из          | В           | Метод              |
|-------------|-------------|-------------------|
| `Draft`     | `Published` | `publish()`       |
| `Published` | `Draft`     | `unpublish()`     |
| `Published` | `Archived`  | `archive(reason)` |
| `Archived`  | `Draft`     | `restore_to_draft()` |

Все остальные комбинации (`Draft → Archived`, `Archived → Published`, self-переходы) возвращают `ContentError::Validation`.

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
