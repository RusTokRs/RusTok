# rustok-workflow — implementation plan

## Scope and objective

Визуальный конструктор автоматизаций (n8n / Directus Flows-style), встроенный в
событийную инфраструктуру платформы. Горизонтальный модуль — оркестрирует взаимодействие
между любыми доменными модулями через события, не создавая собственный event loop.

## Target architecture

- `rustok-workflow` реализует `RusToKModule` (`ModuleKind::Optional`).
- Работает через абстракции `EventBus` / `EventTransport` из `rustok-core`.
- Интегрируется с `alloy-scripting` через шаг `alloy_script`.
- Admin UI — Next.js пакет в `crates/rustok-workflow/ui/admin`.

## Delivery phases

### Phase 1 — Foundation ✅ DONE

- [x] Модель данных: таблицы `workflows`, `workflow_steps`, `workflow_executions`, `workflow_step_executions`.
- [x] SeaORM entities для всех таблиц.
- [x] `WorkflowsMigration` (миграция Phase 1).
- [x] `WorkflowModule` реализует `RusToKModule` + `MigrationSource`.
- [x] `WorkflowService` — CRUD: workflows, steps, executions.
- [x] `WorkflowEngine` — линейное выполнение цепочки шагов с логированием в БД.
- [x] `WorkflowTriggerHandler` — подписка на `DomainEvent`, dispatching matching workflows.
- [x] Базовые шаги: `action`, `emit_event`, `condition`.
- [x] RBAC permissions: `Workflows`, `WorkflowExecutions`.
- [x] Тесты: `module_metadata`, `module_permissions`.

### Phase 2 — Alloy + Advanced Steps ✅ DONE

- [x] Шаг `alloy_script` — интеграция с `alloy-scripting` engine (stub + `ScriptRunner` trait).
- [x] Шаг `http` — внешний HTTP-запрос через `reqwest`.
- [x] Шаг `delay` — отложенное выполнение (scheduled event).
- [x] Шаг `notify` — уведомления (stub + `NotificationSender` trait).
- [x] Cron trigger — `WorkflowCronScheduler` + cron-выражения через крейт `cron`.
- [x] Manual trigger — API-запуск через `WorkflowService::trigger_manual`.
- [x] Error handling: стратегии `stop`, `skip`, `retry`, `fallback_step` (поле `on_error`).
- [x] `ExecutionStatus` / `StepExecutionStatus` enums.

### Phase 3 — Admin UI + GraphQL ✅ DONE

- [x] GraphQL API: queries + mutations для workflows, steps, executions.
- [x] `TenantContext` рефакторинг для GraphQL (изолированный контекст).
- [x] Next.js UI пакет: `crates/rustok-workflow/ui/admin`.
  - [x] `WorkflowsPage` — список workflows с фильтрами.
  - [x] `WorkflowFormPage` — форма создания/редактирования + `WorkflowStepEditor`.
  - [x] `WorkflowDetailPage` — детали + `ExecutionHistory`.
  - [x] Лепестос admin UI (Leptos-компоненты для admin Leptos-приложения).
- [x] `nav.ts` — навигационные элементы модуля для next-admin.

### Phase 4 — Alloy Synergy + Versioning ✅ DONE

- [x] Webhook trigger — входящий HTTP-запрос запускает workflow.
- [x] Версионирование workflow: таблица `workflow_versions`, снэпшот при активации.
- [x] `WorkflowPhase4Migration` (миграция Phase 4).
- [x] `WorkflowVersionEntity` + history UI компонент `VersionHistory`.
- [x] Marketplace шаблонов: `WorkflowTemplate`, `BUILTIN_TEMPLATES`, UI `TemplateGallery`.
- [x] Alloy-генерация workflow: `alloy generate workflow <описание>` → создаёт workflow через API.

## Status

**Все фазы реализованы и смержены в ветку платформы.**

Следующие шаги (backlog):
- Integration-тесты с реальной БД (sqlite in-memory).
- Полная реализация `alloy_script` шага (сейчас stub + trait).
- Полная реализация `notify` шага (сейчас stub + trait).
- DAG вместо линейной цепочки шагов (Phase 5, будущее).
- Системные события `workflow.execution.*` в outbox.
