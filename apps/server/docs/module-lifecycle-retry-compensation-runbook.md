# Runbook: retry/compensation for module lifecycle post-hook failures

This document establishes the operational contract for the situation when `ModuleLifecycleService` has already committed tenant state (`enabled=true/false`), but post-hook (`post_enable`/`post_disable`) ended with an error. This is **not a rollback scenario**: module state is considered committed, and the error is handled as retry/compensation flow through `module_operations`.

## When to apply

Use this runbook if in telemetry/logs or admin lifecycle surface you see:

- `module_operations.status = failed`;
- `error` contains `post-hook` marker;
- tenant state already corresponds to requested transition.

Pre-hook errors (before commit) are not covered here: for them committed state does not change and normal toggle retry after fixing the cause is needed.

## Invariants

1. **Committed state is not rolled back automatically.**
2. **`module_operations` remains the source of truth for audit trail** (including `correlation_id`, `requested_by`, `requested_enabled`).
3. **Retry is performed through the module-owned recovery operation**, exposed by `ModuleLifecycleService::retry_failed_post_hook_operation(...)`, to repeat only post-hook for already committed target-state and create a new journal attempt.
4. **Compensation is performed by separate conscious operation** via `ModuleLifecycleService::compensate_failed_operation(...)` or equivalent canonical toggle in reverse direction, not by hidden rollback inside failed post-hook path.

## Basic diagnostics

### 1) Find problematic operations

Example SQL for tenant + module:

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

Check: latest failed record should have non-null `correlation_id`, and current `tenant_modules.enabled` should already be in requested state.

### 2) Check actual tenant state

```sql
SELECT tenant_id, module_slug, enabled, updated_at
FROM tenant_modules
WHERE tenant_id = '<TENANT_UUID>'
  AND module_slug = '<MODULE_SLUG>';
```

If state does not match expectation, this is not a standard post-hook issue and requires separate incident triage.

### 3) Correlate with application logs/traces

Look for `correlation_id` from `module_operations` in structured logs and tracing spans to confirm root cause of post-hook error (network timeout, downstream 5xx, transient auth/policy glitch, etc.).

## Retry flow (preferred)

Use if cause is transient and hook is idempotent.

1. Ensure root-cause is fixed.
2. Get `ModuleOperationRecoveryPlan` via GraphQL query `moduleOperationRecoveryPlan(operationId: ...)`, list of failed candidates via `failedModuleOperationRecoveryPlans(moduleSlug: ..., limit: ...)` or directly via `ModuleLifecycleService::module_operation_recovery_plan(...)` / `failed_module_operation_recovery_plans(...)`.
3. If `recommended_action = retry_post_hook`, call GraphQL mutation `retryFailedModuleOperationPostHook(operationId: ...)` or `ModuleLifecycleService::retry_failed_post_hook_operation(...)` for failed operation id. Service will check that current effective state still matches `requested_enabled`, and will not re-execute pre-hook or commit tenant state again.
4. Verify that GraphQL mutation returned recovery plan of new operation record with status `committed` (or `failed` if post-hook problem repeated) and new `correlation_id`.

Expected result: successful retry **should not** create duplicate side effects, and journal should show new attempt with new `correlation_id` and same target-state.

## Compensation flow (when retry is impossible)

Use if:

- post-hook side effect partially executed and requires targeted compensation;
- business decision requires returning module to previous state.

Steps:

1. Record decision in incident ticket/change log.
2. Get recovery plan and check `previous_effective_enabled`.
3. Execute GraphQL mutation `compensateFailedModuleOperation(operationId: ...)` or `ModuleLifecycleService::compensate_failed_operation(...)`; service will create new lifecycle operation via canonical toggle to `previous_effective_enabled`.
4. Ensure dependent modules/policy-invariants are not violated before compensating toggle.
5. Check new `module_operations` trail (failed/success) and current `tenant_modules` state.

## Minimum post-incident checklist

- [ ] For each failed post-hook case, `correlation_id` and root cause are recorded.
- [ ] Retry or compensation performed via canonical lifecycle entrypoint (`retryFailedModuleOperationPostHook` / `compensateFailedModuleOperation` GraphQL mutations or service-level `retry_failed_post_hook_operation`/`compensate_failed_operation`, not via bypass/SQL).
- [ ] Journal contains final operation record explaining final state.
- [ ] If failure is systemic/recurring, task created for module owner with reference to failed operations.

## Related contracts

- `apps/server/src/services/module_lifecycle.rs`
- `apps/server/src/models/_entities/module_operations.rs`
- `docs/architecture/modules.md`
- `DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md`
