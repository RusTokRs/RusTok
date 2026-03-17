# rustok-workflow — документация модуля

Визуальная автоматизация на платформенной очереди.

## Назначение

`rustok-workflow` предоставляет визуальный конструктор автоматизаций (аналог n8n / Directus Flows),
встроенный в событийную инфраструктуру платформы. Модуль оркестрирует взаимодействие между
доменными модулями через события, не создавая собственный event loop.

## Архитектура

```
DomainEvent (blog.published, order.paid, ...)
       ↓
  EventBus (outbox → EventTransport)
       ↓
  WorkflowTriggerHandler     ← подписан на события платформы
       ↓
  WorkflowEngine             ← находит matching workflows по tenant + trigger
       ↓
  Step 1 → Step 2 → Step 3  ← линейная цепочка шагов
       ↓         ↓
  каждый шаг может публиковать DomainEvent обратно в outbox
```

Workflow **не владеет** транспортом событий — он работает через абстракции
`EventBus` / `EventTransport` из `rustok-core`. Конкретный транспорт (Iggy, RabbitMQ,
базовый Outbox) не имеет значения для модуля.

## Модель данных

| Таблица | Назначение |
|---------|-----------|
| `workflows` | Определение workflow: триггер, статус, tenant |
| `workflow_versions` | Версионирование: снэпшот steps + config каждой версии |
| `workflow_steps` | Шаги workflow: тип, конфиг, порядок, обработка ошибок |
| `workflow_executions` | Журнал запусков: статус, контекст, ошибка |
| `workflow_step_executions` | Журнал выполнения каждого шага в рамках запуска |

## Типы триггеров

| Тип | Источник |
|-----|---------|
| `event` | `DomainEvent` через `EventBus` |
| `cron` | Расписание (cron-выражение), тик через `WorkflowCronScheduler` |
| `webhook` | Входящий HTTP-запрос на платформенный эндпоинт |
| `manual` | Кнопка в админке / API-вызов |

## Типы шагов

| Тип | Что делает |
|-----|-----------|
| `action` | Вызывает действие платформенного сервиса |
| `emit_event` | Публикует `DomainEvent` в outbox |
| `condition` | Ветвление по значению в JSON-контексте |
| `delay` | Отложенное выполнение через scheduled event |
| `http` | Внешний HTTP-запрос (webhook out) |
| `alloy_script` | Запускает Rhai-скрипт через `alloy-scripting` |
| `notify` | Уведомление (email, Slack, Telegram) |

## Связь с Alloy

Workflow оркестрирует — Alloy исполняет. Alloy может быть шагом внутри workflow:

```
Trigger: order.paid
  → Step 1: alloy_script "сгенерируй invoice PDF"
  → Step 2: notify — отправь email клиенту
  → Step 3: http — уведомить CRM
```

В перспективе Alloy может порождать workflow из описания на натуральном языке.

## RBAC

Ресурс `Workflows`: `Create`, `Read`, `Update`, `Delete`, `List`, `Execute`, `Manage`.
Ресурс `WorkflowExecutions`: `Read`, `List`.

Все таблицы содержат `tenant_id` — полная изоляция между тенантами.

## Admin UI

Пакет: `crates/rustok-workflow/ui/admin` (Next.js).

Экраны:
- Список workflows с фильтрами по статусу и тригеру
- Форма создания/редактирования workflow + редактор шагов
- Детальная страница: описание, история выполнений, версии
- Галерея шаблонов (marketplace templates)
- История версий с diff между версиями

## Связанные документы

- [Implementation plan](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Архитектурное описание](../../../docs/architecture/workflow.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)

## Event contracts

Модуль публикует события через `emit_event`-шаг. Контракт событий определяется
конфигурацией конкретного workflow — не зашит в код модуля.

Системные события (будущее):
- `workflow.execution.started`
- `workflow.execution.completed`
- `workflow.execution.failed`

## Roadmap / статус

- ✅ Phase 1 — Foundation: таблицы, entities, `WorkflowService`, `WorkflowEngine`, event trigger, базовые шаги
- ✅ Phase 2 — Advanced Steps: `alloy_script`, `http`, `delay`, `notify`, cron trigger, manual trigger, error handling
- ✅ Phase 3 — Admin UI: граф-редактор в Next.js, execution history, Leptos GraphQL API
- ✅ Phase 4 — Alloy Synergy: webhook trigger, версионирование, marketplace шаблоны, Alloy-генерация workflow
