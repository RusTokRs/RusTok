# Workflow — визуальная автоматизация на платформенной очереди

> **Статус:** Реализован ✅
> **Модуль:** `rustok-workflow` ([docs](../../crates/rustok-workflow/docs/README.md) · [plan](../../crates/rustok-workflow/docs/implementation-plan.md) · [CRATE_API](../../crates/rustok-workflow/CRATE_API.md))
> **Вид:** `ModuleKind::Optional` — полноценный модуль с таблицами, зарегистрирован в `apps/server`

---

## 1. Что это

Визуальный конструктор автоматизаций (аналог n8n / Directus Flows), но:
- Не параллельная система очередей — встраивается в существующую event-инфраструктуру (outbox → `EventTransport`)
- Не внешний сервис — часть платформы с tenant isolation и RBAC
- Не замена Alloy — дополнение: Workflow оркестрирует, Alloy исполняет произвольную логику

---

## 2. Архитектурный принцип

```
DomainEvent (blog.published, order.paid, ...)
       ↓
  EventBus (outbox → EventTransport)
       ↓
  WorkflowTriggerHandler          ← подписан на события
       ↓
  WorkflowEngine                  ← находит matching workflows
       ↓
  Step 1 → Step 2 → Step 3       ← цепочка шагов
       ↓         ↓         ↓
  каждый шаг может эмитить DomainEvent обратно в outbox
```

**Ключевое:** workflow НЕ запускает свой event loop. Он — потребитель и производитель событий
через существующий `TransactionalEventBus`. Конкретный транспорт (iggy, rabbitmq, базовый outbox)
workflow не волнует — он работает через абстракции `EventBus` / `EventTransport`.
Вся инфраструктура доставки (retry, DLQ, replay) обеспечивается транспортным слоем.

---

## 3. Workflow и Alloy

| | Workflow | Alloy |
|--|----------|-------|
| **Что делает** | Оркестрирует: "когда X → сделай Y, потом Z" | Исполняет: произвольная логика на данных |
| **Интерфейс** | Визуальный редактор (граф шагов) | Натуральный язык / Rhai / API |
| **Уровень** | Координация между модулями | Создание новой функциональности |
| **Данные** | Маршрутизирует между шагами | Трансформирует, обогащает, создаёт |

**Пересечение:** Alloy может быть action-шагом внутри workflow.
Workflow запускает Alloy-скрипт как один из шагов цепочки.

```
Trigger: order.paid
  → Step 1: Alloy script "сгенерируй invoice PDF"
  → Step 2: отправь email клиенту
  → Step 3: обнови статус в CRM
  → Step 4: Alloy script "синхронизируй с 1С"
```

В будущем Alloy может **порождать workflow** — описал на человеческом языке бизнес-процесс,
Alloy создал граф workflow с нужными шагами и Rhai-скриптами внутри.

---

## 4. Модель данных (planned)

### Таблицы

```
workflows
├── id: uuid
├── tenant_id: uuid
├── name: string
├── description: text
├── status: enum (draft | active | paused | archived)
├── trigger_config: jsonb    -- тип триггера + параметры
├── created_at / updated_at
└── created_by: uuid

workflow_steps
├── id: uuid
├── workflow_id: uuid (FK)
├── position: i32            -- порядок выполнения
├── step_type: enum          -- action | condition | delay | alloy_script | ...
├── config: jsonb            -- параметры шага
├── on_error: enum           -- stop | skip | retry | fallback_step
└── timeout_ms: i64

workflow_executions
├── id: uuid
├── workflow_id: uuid (FK)
├── trigger_event_id: uuid   -- какое событие запустило
├── status: enum (running | completed | failed | timed_out)
├── started_at / completed_at
├── context: jsonb           -- данные, переданные между шагами
└── error: text

workflow_step_executions
├── id: uuid
├── execution_id: uuid (FK)
├── step_id: uuid (FK)
├── status: enum (pending | running | completed | failed | skipped)
├── input: jsonb
├── output: jsonb
├── started_at / completed_at
└── error: text
```

---

## 5. Типы триггеров

| Тип | Источник | Пример |
|-----|----------|--------|
| **Event** | `DomainEvent` через `EventBus` | `blog.post.published`, `commerce.order.paid` |
| **Cron** | Планировщик (как в Alloy) | "каждый день в 02:00" |
| **Webhook** | Входящий HTTP-запрос | внешний сервис вызывает endpoint |
| **Manual** | Кнопка в админке | оператор запускает вручную |
| **Alloy** | Alloy-скрипт вызывает `workflow.trigger()` | программный запуск из скрипта |

---

## 6. Типы шагов

| Тип | Что делает |
|-----|-----------|
| **action** | Вызывает сервис модуля (через трейт) |
| **alloy_script** | Запускает Rhai-скрипт через Alloy engine |
| **condition** | Ветвление: if/else по данным контекста |
| **delay** | Ожидание (реализуется через scheduled event в `EventTransport`) |
| **emit_event** | Публикует `DomainEvent` в outbox |
| **http** | Внешний HTTP-вызов (webhook out) |
| **notify** | Уведомление (email, Telegram, Slack — через интеграции) |
| **transform** | Трансформация данных контекста (map/filter/merge) |

---

## 7. Интеграция с платформой

### Event System

```rust
// WorkflowTriggerHandler подписывается на ВСЕ события
// и проверяет, есть ли активные workflows с matching trigger
impl EventHandler for WorkflowTriggerHandler {
    async fn handle(&self, event: &EventEnvelope) -> Result<()> {
        let workflows = self.registry
            .find_by_trigger(event.event_type(), event.tenant_id())
            .await?;

        for workflow in workflows {
            self.engine.execute(workflow, event.into()).await?;
        }
        Ok(())
    }
}
```

### Шаг эмитит событие обратно

```rust
// Внутри шага emit_event — публикация через outbox
self.event_bus.publish_in_tx(
    &txn,
    DomainEvent::new("workflow.step.completed", payload),
).await?;
```

### RBAC

Ресурс `Workflows` с permissions: `Create`, `Read`, `Update`, `Delete`, `Execute`, `Manage`.

### Tenant Isolation

Все таблицы с `tenant_id`. Workflow видит только события своего тенанта.

---

## 8. UI

Визуальный редактор workflow (граф) — в админке:
- Drag & drop шагов
- Настройка триггеров
- Просмотр истории выполнений
- Логи каждого шага
- Кнопка manual trigger

Технология: Next.js (apps/next-admin) — `crates/rustok-workflow/ui/admin-next`.

---

## 9. Статус реализации

Все четыре фазы реализованы. Детальный план с чеклистами:
[`crates/rustok-workflow/docs/implementation-plan.md`](../../crates/rustok-workflow/docs/implementation-plan.md)

---

## 10. Архитектурные решения

| Решение | Выбор | Обоснование |
|---------|-------|-------------|
| Очередь | Существующий `EventBus` / `EventTransport` | Не дублировать инфраструктуру. Transport-agnostic: iggy, rabbitmq, базовый outbox |
| Хранение | SeaORM entities + JSONB config | Единообразно с другими модулями |
| Шаги | trait `WorkflowStep` | Расширяемость, модули могут регистрировать свои шаги |
| Execution | Async, каждый шаг — отдельный span | Observability через существующий telemetry |
| Alloy | Alloy-шаг = вызов `ScriptOrchestrator` | Переиспользование, не дублирование |
| Визуальный граф | Хранится как ordered steps + conditions | Простота первой версии, DAG — позже |
| Delayed steps | Scheduled events через `EventTransport` | Не таймеры в памяти. Работает с любым транспортом |

---

## 11. Связь с другими модулями

```
rustok-workflow
  ├── зависит от: rustok-core (трейты, registry, events, EventBus, EventTransport)
  ├── интегрируется с: alloy-scripting (шаг alloy_script)
  ├── использует: rustok-outbox (publish_in_tx) — через абстракцию EventBus
  ├── использует: rustok-cache (кэш активных workflows)
  └── используется: любым модулем через event triggers

НЕ зависит напрямую от конкретного транспорта (iggy, rabbitmq, etc.)
— только от абстракций EventBus / EventTransport из rustok-core.
```

> Workflow — горизонтальный модуль, как Alloy. Он не привязан к одному домену —
> он оркестрирует взаимодействие между любыми модулями через события.
