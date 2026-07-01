# rustok-rbac / CRATE_API

## Публичные модули
`dto`, `entities`, `error`, `services`.

## Основные публичные типы и сигнатуры
- `pub struct RbacModule`
- Публичные DTO/сервисы RBAC из `services`.
- Контракты авторизации переиспользуются из `rustok_api` и `rustok_core::rbac`.
- Re-export policy helpers at crate root:
  - `has_effective_permission_in_set`
  - `missing_permissions`
  - `check_permission`
  - `check_any_permission`
  - `check_all_permissions`
  - `PermissionCheckOutcome`
  - `denied_reason_for_denial`
  - `DeniedReasonKind`

## События
- Публикует: как правило не публикует бизнес-события по умолчанию.
- Потребляет: N/A (вызов сервисов напрямую).

## Зависимости от других rustok-крейтов
- `rustok-core`

## Частые ошибки ИИ
- Путает `Resource/Action/Permission` из core с локальными DTO.
- Добавляет проверку прав в неправильном слое (вместо application/service boundary).

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
