# Runbook: retry/compensation for module lifecycle post-hook failures

Документ фиксирует операционный контракт для ситуации, когда `ModuleLifecycleService` уже закоммитил tenant state (`enabled=true/false`), но post-hook (`post_enable`/`post_disable`) завершился ошибкой. Это **не rollback-сценарий**: состояние модуля считается committed, а ошибка обслуживается как retry/compensation поток через `module_operations`.

## Когда применять

Используйте этот runbook, если в telemetry/логах или в admin lifecycle surface видите:

- `module_operations.status = failed`;
- `error` содержит маркер `post-hook`;
- tenant state уже соответствует requested transition.

Pre-hook ошибки (до commit) сюда не относятся: для них committed state не меняется и нужен обычный повтор toggle после исправления причины.

## Инварианты

1. **Committed state не откатывается автоматически.**
2. **`module_operations` остаётся источником истины для audit trail** (включая `correlation_id`, `requested_by`, `requested_enabled`).
3. **Retry выполняется через canonical lifecycle service**, предпочтительно через `ModuleLifecycleService::retry_failed_post_hook_operation(...)`, чтобы повторить только post-hook для уже committed target-state и создать новый journal attempt.
4. **Compensation выполняется отдельной осознанной операцией** через `ModuleLifecycleService::compensate_failed_operation(...)` или эквивалентный canonical toggle в обратную сторону, а не скрытым rollback внутри failed post-hook path.

## Базовая диагностика

### 1) Найти проблемные операции

Пример SQL для tenant + module:

```sql
SELECT id,
       tenant_id,
       module_slug,
       requested_enabled,
       status,
       correlation_id,
       requested_by,
       error,
       created_at,
       updated_at
FROM module_operations
WHERE tenant_id = '<TENANT_UUID>'
  AND module_slug = '<MODULE_SLUG>'
ORDER BY created_at DESC;
```

Проверка: у latest failed-записи должен быть non-null `correlation_id`, а текущий `tenant_modules.enabled` — уже в requested состоянии.

### 2) Проверить фактическое tenant state

```sql
SELECT tenant_id, module_slug, enabled, updated_at
FROM tenant_modules
WHERE tenant_id = '<TENANT_UUID>'
  AND module_slug = '<MODULE_SLUG>';
```

Если state не соответствует ожиданию, это не штатный post-hook issue и требует отдельного incident triage.

### 3) Коррелировать с application logs/traces

Ищите `correlation_id` из `module_operations` в structured logs и tracing span-ах для подтверждения root cause post-hook ошибки (network timeout, downstream 5xx, transient auth/policy glitch и т.д.).

## Retry flow (предпочтительный)

Используйте, если причина transient и hook idempotent.

1. Убедитесь, что root-cause устранён.
2. Получите `ModuleOperationRecoveryPlan` через `ModuleLifecycleService::module_operation_recovery_plan(...)` или список failed candidates через `failed_module_operation_recovery_plans(...)`.
3. Если `recommended_action = retry_post_hook`, вызовите `ModuleLifecycleService::retry_failed_post_hook_operation(...)` для failed operation id. Сервис проверит, что текущий effective state всё ещё совпадает с `requested_enabled`, и не будет заново выполнять pre-hook или commit tenant state.
4. Проверьте, что создана новая operation-запись со статусом `committed` (или `failed`, если post-hook проблема повторилась) и новым `correlation_id`.

Ожидаемый результат: success retry **не должен** создавать duplicate side effects, а журнал должен показать новый attempt с новым `correlation_id` и тем же target-state.

## Compensation flow (когда retry невозможен)

Используйте, если:

- post-hook side effect частично выполнился и требует целенаправленной компенсации;
- бизнес-решение требует вернуть модуль в предыдущее состояние.

Шаги:

1. Зафиксировать решение в incident ticket/change log.
2. Получить recovery plan и проверить `previous_effective_enabled`.
3. Выполнить `ModuleLifecycleService::compensate_failed_operation(...)`; сервис создаст новую lifecycle operation через canonical toggle к `previous_effective_enabled`.
4. Убедиться, что зависимые модули/policy-инварианты не нарушены перед compensating toggle.
5. Проверить новый `module_operations` trail (failed/success) и текущее состояние `tenant_modules`.

## Минимальный post-incident checklist

- [ ] Для каждого failed post-hook случая зафиксирован `correlation_id` и root cause.
- [ ] Retry или compensation выполнены через canonical lifecycle entrypoint (`retry_failed_post_hook_operation`/`compensate_failed_operation`, не через bypass/SQL).
- [ ] В журнале есть финальная operation-запись, объясняющая конечное состояние.
- [ ] Если сбой системный/повторяющийся, создана задача на модуль-владельца с ссылкой на failed operations.

## Связанные контракты

- `apps/server/src/services/module_lifecycle.rs`
- `apps/server/src/models/_entities/module_operations.rs`
- `docs/research/control-plane-module-lifecycle-remediation-plan.md`
- `DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md`
