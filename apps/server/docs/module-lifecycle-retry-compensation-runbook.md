# Runbook: retry/compensation for module lifecycle post-hook failures

This document establishes the operational contract for the situation when `ModuleLifecycleService` has already committed tenant module intent, but post-hook (`post_enable`/`post_disable`) ended with an error. This is **not an automatic rollback scenario**: the requested tenant override is committed first, and the error is handled as retry/compensation flow through `module_operations` plus its immutable selected-intent recovery state.

Effective serving availability is not the recovery predecessor. Availability can legitimately differ from tenant intent while Product co-requisites are staged, channel/maintenance policy is applied, or node readiness changes.

## When to apply

Use this runbook if in telemetry/logs or admin lifecycle surface you see:

- `module_operations.status = failed`;
- `error` contains `post-hook` marker;
- the exact current tenant override still corresponds to the override target recorded for that operation.

Pre-hook errors (before commit) are not covered here: for them committed tenant intent does not change and normal toggle retry after fixing the cause is needed.

## Selected-intent states

Lifecycle recovery distinguishes three exact tenant override states:

- `inherit` — there is no `tenant_modules` row for the module (`None` in the owner contract);
- `enabled` — an explicit `tenant_modules.enabled = true` override exists;
- `disabled` — an explicit `tenant_modules.enabled = false` override exists.

For new lifecycle operations the owner stores the exact predecessor and requested override in `module_operation_override_states`. The side-table row itself is the recorded-state marker, so nullable booleans can represent `inherit` without confusing it with a legacy operation that predates this contract.

Legacy failed operations without a `module_operation_override_states` row fail closed for post-hook retry/compensation with `selected_intent_state_unavailable`; do not infer their predecessor from `previous_effective_enabled`.

## Invariants

1. **Committed tenant intent is not rolled back automatically.**
2. **`module_operations` remains the audit trail** for operation direction, correlation, actor and historical effective availability. `previous_effective_enabled` is an availability fact, not the selected-intent recovery predecessor.
3. **`module_operation_override_states` is immutable recovery evidence** for the exact explicit-override predecessor and target.
4. **Retry repeats only the post-hook** after proving the current explicit override still equals the original requested override. It does not require serving availability to equal `requested_enabled`.
5. **Compensation is a new canonical lifecycle operation** in the inverse hook/dependency-order direction. Its state target is the exact recorded predecessor; `inherit` removes the explicit tenant override row.
6. **Policy revision remains canonical and co-requisite-aware.** Compensation computes the next policy from the exact restored override set and publishes the normal owner policy transition/outbox revision.

## Basic diagnostics

### 1) Find problematic operations

Example SQL for tenant + module:

```sql
SELECT id,
       tenant_id,
       module_slug,
       requested_enabled,
       previous_effective_enabled,
       status,
       correlation_id,
       requested_by,
       error_message,
       created_at,
       updated_at
FROM module_operations
WHERE tenant_id = '<TENANT_UUID>'
  AND module_slug = '<MODULE_SLUG>'
ORDER BY created_at DESC;
```

For an operation created under the current recovery contract, inspect its exact selected-intent evidence:

```sql
SELECT operation_id,
       previous_override_enabled,
       requested_override_enabled,
       created_at
FROM module_operation_override_states
WHERE operation_id = '<OPERATION_UUID>';
```

Interpret nullable override values only when this row exists. `NULL` then means `inherit`.

### 2) Check actual tenant override

```sql
SELECT tenant_id, module_slug, enabled, settings, updated_at
FROM tenant_modules
WHERE tenant_id = '<TENANT_UUID>'
  AND module_slug = '<MODULE_SLUG>';
```

No row means current selected intent is `inherit`. A row means explicit `enabled` or `disabled`. If this exact state does not match the operation's `requested_override_enabled`, retry/compensation must fail with a state mismatch and the incident needs separate triage.

Do not substitute `EffectiveModulePolicyService` availability for this check: Product may be explicitly enabled while intentionally unavailable until Inventory/Pricing co-requisites are also selected.

### 3) Correlate with application logs/traces

Look for `correlation_id` from `module_operations` in structured logs and tracing spans to confirm root cause of post-hook error (network timeout, downstream 5xx, transient auth/policy glitch, etc.).

## Retry flow (preferred)

Use if cause is transient and the post-hook is idempotent.

1. Ensure root cause is fixed.
2. Get `ModuleOperationRecoveryPlan` via GraphQL query `moduleOperationRecoveryPlan(operationId: ...)`, the failed candidate list, or the service owner API.
3. Confirm the plan is retryable. Operations without recorded selected-intent state are deliberately not retryable through this contract.
4. Call `retryFailedModuleOperationPostHook` or `ModuleLifecycleService::retry_failed_post_hook_operation(...)`.
5. The owner verifies the exact current override equals the original requested override, creates a new journal attempt, copies the original selected-intent predecessor/target recovery evidence, and dispatches only the post-hook. It does not re-run pre-hook or persist tenant state again.
6. Verify the new operation record is `committed`, or `failed` with a new post-hook error and correlation id.

Expected result: successful retry **should not** create duplicate side effects, and the new journal attempt retains enough selected-intent evidence to be compensated later if its own post-hook fails.

## Compensation flow (when retry is impossible)

Use if:

- the post-hook side effect partially executed and requires a deliberate reverse lifecycle operation; or
- business decision requires restoring the exact selected intent that existed before the failed operation.

Steps:

1. Record the decision in the incident ticket/change log.
2. Get the recovery plan and verify selected-intent evidence was recorded. Do **not** use `previous_effective_enabled` as the compensation target.
3. Confirm the current explicit override still equals the original operation's requested override.
4. Execute `compensateFailedModuleOperation` or `ModuleLifecycleService::compensate_failed_operation(...)`.
5. The owner runs the inverse lifecycle hook/dependency-order direction and restores `previous_override_enabled` exactly:
   - `Some(true)` -> explicit enabled row;
   - `Some(false)` -> explicit disabled row;
   - `None` -> remove the explicit row and return to inherited/default selection.
6. The owner computes/publishes the resulting canonical effective-policy revision from that exact restored override set.
7. Check the new `module_operations` trail and the current `tenant_modules` row/absence. Serving availability may still differ from selected intent because co-requisites or other policy context can make the module unavailable.

## Minimum post-incident checklist

- [ ] For each failed post-hook case, `correlation_id` and root cause are recorded.
- [ ] Recovery plan has selected-intent evidence; legacy rows without it were not guessed from effective availability.
- [ ] Retry or compensation was performed through the canonical lifecycle entrypoint, not bypass SQL.
- [ ] The exact explicit override after recovery is correct, including expected row absence for inherited selection.
- [ ] Canonical effective-policy revision/outbox reflects the resulting selection.
- [ ] Journal contains the final operation record explaining final lifecycle direction and result.
- [ ] If failure is systemic/recurring, a task exists for the module owner with references to failed operations.

## Related contracts

- `crates/rustok-modules/src/recovery.rs`
- `crates/rustok-modules/src/executor.rs`
- `crates/rustok-modules/src/lifecycle_writer.rs`
- `crates/rustok-migrations/src/m20260808_000099_create_module_operation_override_states.rs`
- `apps/server/src/services/module_lifecycle.rs`
- `docs/architecture/modules.md`
- `DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md`
