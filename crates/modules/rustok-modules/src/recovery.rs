use rustok_api::{ModuleOperationRecoveryPlanView, StaticModuleSettingsReader};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use thiserror::Error;

use crate::backend::SupportedBackend;
use crate::executor::{
    ClaimHeartbeatDispatchError, dispatch_lifecycle_with_claim_heartbeat,
};
use crate::{
    DatabaseStaticModuleSettingsReader, ModuleCommandContext, ModuleControlPlaneError,
    ModuleDefinitionSource,
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, ModuleOperationIssue,
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecordOutcome,
    ModuleOperationRecoveryAction, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStatus, ModuleOperationStoreError, StaticTenantLifecycleStore,
    StaticTenantLifecycleStoreError, TenantModuleStateRecord, TenantModuleStateRequest,
    TenantModuleStateStore, artifact_settings,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModuleOperationOverrideState {
    pub previous_enabled: Option<bool>,
    pub requested_enabled: Option<bool>,
}

/// Transport-neutral recovery view of a failed lifecycle operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOperationRecoveryPlan {
    pub operation_id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub override_state_recorded: bool,
    pub previous_override_enabled: Option<bool>,
    pub requested_override_enabled: Option<bool>,
    pub status: ModuleOperationStatus,
    pub issue: ModuleOperationIssue,
    pub retryable: bool,
    pub recommended_action: ModuleOperationRecoveryAction,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub requested_by: Option<String>,
    pub error_message: Option<String>,
}

impl From<ModuleOperationRecoveryPlan> for ModuleOperationRecoveryPlanView {
    fn from(plan: ModuleOperationRecoveryPlan) -> Self {
        Self {
            operation_id: plan.operation_id.to_string(),
            tenant_id: plan.tenant_id.to_string(),
            module_slug: plan.module_slug,
            requested_enabled: plan.requested_enabled,
            previous_effective_enabled: plan.previous_effective_enabled,
            status: plan.status.as_str().to_string(),
            issue: plan.issue.as_str().to_string(),
            retryable: plan.retryable,
            recommended_action: plan.recommended_action.as_str().to_string(),
            correlation_id: plan.correlation_id,
            requested_by: plan.requested_by,
            error_message: plan.error_message,
        }
    }
}

impl ModuleOperationRecoveryPlan {
    fn from_snapshot(
        operation: ModuleOperationSnapshot,
        override_state: Option<ModuleOperationOverrideState>,
    ) -> Self {
        let issue = match (operation.status, operation.error_message.as_deref()) {
            // Typed outcome recorded by the executor since the
            // committed/hook-failed status split.
            (ModuleOperationStatus::CommittedHookFailed, _) => ModuleOperationIssue::PostHookFailed,
            // Legacy rows: post-hook failures were recorded as `failed` with
            // a `post-hook:` message prefix before the status split.
            (ModuleOperationStatus::Failed, Some(message)) if message.starts_with("post-hook:") => {
                ModuleOperationIssue::PostHookFailed
            }
            (ModuleOperationStatus::Failed, Some(message))
                if message.starts_with("state-commit:")
                    || message.starts_with("recovery-state:") =>
            {
                ModuleOperationIssue::OtherFailed
            }
            (ModuleOperationStatus::Failed, Some(message)) if !message.is_empty() => {
                ModuleOperationIssue::PreHookFailed
            }
            (ModuleOperationStatus::Failed, _) => ModuleOperationIssue::OtherFailed,
            (ModuleOperationStatus::Orphaned, _) => ModuleOperationIssue::Orphaned,
            _ => ModuleOperationIssue::None,
        };
        let override_state_recorded = override_state.is_some();
        let retryable = issue.retryable() && override_state_recorded;
        let recommended_action = if retryable {
            ModuleOperationRecoveryAction::RetryPostHook
        } else if matches!(
            issue,
            ModuleOperationIssue::PreHookFailed | ModuleOperationIssue::Orphaned
        ) {
            ModuleOperationRecoveryAction::RepeatToggle
        } else {
            ModuleOperationRecoveryAction::None
        };
        Self {
            operation_id: operation.id,
            tenant_id: operation.tenant_id,
            module_slug: operation.module_slug,
            requested_enabled: operation.requested_enabled,
            previous_effective_enabled: operation.previous_effective_enabled,
            override_state_recorded,
            previous_override_enabled: override_state.and_then(|state| state.previous_enabled),
            requested_override_enabled: override_state.and_then(|state| state.requested_enabled),
            status: operation.status,
            issue,
            retryable,
            recommended_action,
            correlation_id: operation.correlation_id,
            trace_id: operation.trace_id,
            requested_by: operation.requested_by,
            error_message: operation.error_message,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ModulePostHookRetryRequest {
    pub operation_id: uuid::Uuid,
    /// Authenticated tenant-scoped command evidence retained by the retry
    /// journal entry. Recovery retries never synthesize a new correlation.
    pub context: ModuleCommandContext,
    /// Revision reviewed by the actor before retrying the lifecycle
    /// operation. Recovery acquires the same tenant/module claim as a
    /// toggle, then rereads the override and settings under that claim.
    pub expected_revision: u64,
}

#[derive(Debug, Error)]
pub enum ModuleOperationRecoveryError {
    #[error("module operation not found")]
    OperationNotFound,
    #[error("module recovery command identity must not be nil")]
    InvalidCommandIdentity,
    #[error(transparent)]
    InvalidCommandContext(#[from] ModuleControlPlaneError),
    #[error("module operation idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("module lifecycle revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("module lifecycle operation is already active")]
    OperationInProgress,
    #[error("module lifecycle claim was lost")]
    ClaimLost,
    #[error("module operation is not retryable: {0}")]
    NotRetryable(String),
    #[error(
        "module operation override state mismatch: requested={requested_override_enabled:?}, current={current_override_enabled:?}"
    )]
    StateMismatch {
        requested_override_enabled: Option<bool>,
        current_override_enabled: Option<bool>,
    },
    #[error("module post-hook retry failed: {0}")]
    PostHookFailed(String),
    #[error("module operation persistence failed: {0}")]
    Persistence(String),
}

pub(crate) async fn module_operation_recovery_plan(
    db: &DatabaseConnection,
    operation_id: uuid::Uuid,
) -> Result<ModuleOperationRecoveryPlan, ModuleOperationRecoveryError> {
    let operation = ModuleOperationJournal::find(db, operation_id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?
        .ok_or(ModuleOperationRecoveryError::OperationNotFound)?;
    let override_state = operation_override_state(db, operation_id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
    Ok(ModuleOperationRecoveryPlan::from_snapshot(
        operation,
        override_state,
    ))
}

pub(crate) async fn failed_module_operation_recovery_plans(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: Option<&str>,
) -> Result<Vec<ModuleOperationRecoveryPlan>, ModuleOperationRecoveryError> {
    let operations = ModuleOperationJournal::failed_for_tenant(db, tenant_id, module_slug)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
    let mut plans = Vec::with_capacity(operations.len());
    for operation in operations {
        let override_state = operation_override_state(db, operation.id)
            .await
            .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
        plans.push(ModuleOperationRecoveryPlan::from_snapshot(
            operation,
            override_state,
        ));
    }
    Ok(plans)
}

/// Loads the settings owned by the definition's implementation class at
/// claim time. Lifecycle hooks must not consume a caller-side snapshot read
/// before the aggregate claim was acquired.
pub(crate) async fn load_current_lifecycle_settings(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    tenant_id: uuid::Uuid,
    module_slug: &str,
) -> Result<serde_json::Value, ModuleOperationStoreError> {
    let definition = dispatcher.catalog().get(module_slug).ok_or_else(|| {
        ModuleOperationStoreError::Database(format!(
            "module definition `{module_slug}` is unavailable while loading lifecycle settings"
        ))
    })?;
    if matches!(&definition.source, ModuleDefinitionSource::Artifact { .. }) {
        return artifact_settings::load(db, tenant_id, module_slug)
            .await
            .map_err(store_database_error);
    }
    let reader = DatabaseStaticModuleSettingsReader::new(db.clone());
    let snapshot = reader
        .settings(tenant_id, module_slug)
        .await
        .map_err(store_database_error)?;
    Ok(snapshot
        .map(|snapshot| snapshot.settings)
        .unwrap_or_else(|| serde_json::json!({})))
}

fn retry_operation_request(
    plan: &ModuleOperationRecoveryPlan,
    context: &ModuleCommandContext,
    expected_revision: u64,
) -> ModuleOperationRequest {
    ModuleOperationRequest {
        tenant_id: plan.tenant_id,
        module_slug: plan.module_slug.clone(),
        requested_enabled: plan.requested_enabled,
        previous_effective_enabled: plan.previous_effective_enabled,
        requested_by: Some(context.actor_id.to_string()),
        trace_id: Some(context.trace_id.clone()),
        correlation_id: context.correlation_id.to_string(),
        idempotency_key: Some(context.idempotency_key),
        expected_revision: Some(expected_revision),
    }
}

pub(crate) async fn retry_failed_post_hook_operation(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    request: ModulePostHookRetryRequest,
) -> Result<ModuleOperationRecord, ModuleOperationRecoveryError> {
    if request.context.validate().is_err() {
        return Err(ModuleOperationRecoveryError::InvalidCommandIdentity);
    }
    let plan = module_operation_recovery_plan(db, request.operation_id).await?;
    if request.context.tenant_id != Some(plan.tenant_id) {
        return Err(ModuleOperationRecoveryError::InvalidCommandIdentity);
    }
    let journal_request =
        retry_operation_request(&plan, &request.context, request.expected_revision);
    if let Some(operation) = ModuleOperationJournal::replay_idempotent(db, &journal_request)
        .await
        .map_err(|error| match error {
            crate::ModuleOperationStoreError::IdempotencyConflict => {
                ModuleOperationRecoveryError::IdempotencyConflict
            }
            error => ModuleOperationRecoveryError::Persistence(error.to_string()),
        })?
    {
        return Ok(operation);
    }
    if !plan.override_state_recorded {
        return Err(ModuleOperationRecoveryError::NotRetryable(
            "selected_intent_state_unavailable".to_string(),
        ));
    }
    if !plan.retryable {
        return Err(ModuleOperationRecoveryError::NotRetryable(
            plan.issue.to_string(),
        ));
    }
    if dispatcher.catalog().get(&plan.module_slug).is_none() {
        return Err(ModuleOperationRecoveryError::NotRetryable(
            "unknown_module".to_string(),
        ));
    }
    let operation = match ModuleOperationJournal::record_idempotent(db, journal_request)
        .await
        .map_err(|error| match error {
            crate::ModuleOperationStoreError::IdempotencyConflict => {
                ModuleOperationRecoveryError::IdempotencyConflict
            }
            error => ModuleOperationRecoveryError::Persistence(error.to_string()),
        })? {
        ModuleOperationRecordOutcome::Recorded(operation) => operation,
        ModuleOperationRecordOutcome::Replayed(operation) => return Ok(operation),
    };
    if let Err(error) = StaticTenantLifecycleStore::claim(
        db,
        plan.tenant_id,
        &plan.module_slug,
        request.expected_revision,
        request.context.idempotency_key,
    )
    .await
    {
        let contention = matches!(
            error,
            StaticTenantLifecycleStoreError::RevisionConflict { .. }
                | StaticTenantLifecycleStoreError::OperationInProgress { .. }
        );
        let mapped = map_lifecycle_claim_error(error);
        if contention {
            return Err(
                match ModuleOperationJournal::discard_validated(db, operation.id).await {
                    Ok(true) => mapped,
                    Ok(false) => ModuleOperationRecoveryError::Persistence(format!(
                        "{mapped}; additionally, the unclaimed recovery operation \
                         was no longer in the validated state"
                    )),
                    Err(discard_error) => ModuleOperationRecoveryError::Persistence(format!(
                        "{mapped}; additionally, discarding the unclaimed recovery \
                         operation failed: {discard_error}"
                    )),
                },
            );
        }
        let message = format!("aggregate-claim: {mapped}");
        return Err(
            match ModuleOperationJournal::mark_failed(db, operation.id, &message).await {
                Ok(()) => mapped,
                Err(journal_error) => ModuleOperationRecoveryError::Persistence(format!(
                    "{mapped}; additionally, marking the recovery operation failed \
                     did not persist: {journal_error}"
                )),
            },
        );
    }

    let claimed_result = retry_failed_post_hook_operation_claimed(
        db,
        dispatcher,
        &request,
        plan.operation_id,
        operation.clone(),
    )
    .await;
    let release_result = StaticTenantLifecycleStore::release(
        db,
        plan.tenant_id,
        &plan.module_slug,
        request.context.idempotency_key,
    )
    .await
    .map_err(map_lifecycle_claim_error);
    match (claimed_result, release_result) {
        (Ok(operation), Ok(())) => Ok(operation),
        (Ok(_), Err(release_error)) => Err(release_error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(release_error)) => {
            Err(ModuleOperationRecoveryError::Persistence(format!(
                "{error}; additionally failed to release the lifecycle claim \
                 (it will expire via its lease): {release_error}"
            )))
        }
    }
}

async fn retry_failed_post_hook_operation_claimed(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    request: &ModulePostHookRetryRequest,
    original_operation_id: uuid::Uuid,
    operation: ModuleOperationRecord,
) -> Result<ModuleOperationRecord, ModuleOperationRecoveryError> {
    let plan = match module_operation_recovery_plan(db, original_operation_id).await {
        Ok(plan) => plan,
        Err(error) => {
            return Err(mark_recovery_operation_failed(db, operation.id, error).await);
        }
    };
    if request.context.tenant_id != Some(plan.tenant_id) {
        return Err(discard_unstarted_recovery_operation(
            db,
            operation.id,
            ModuleOperationRecoveryError::InvalidCommandIdentity,
        )
        .await);
    }
    if !plan.override_state_recorded {
        return Err(discard_unstarted_recovery_operation(
            db,
            operation.id,
            ModuleOperationRecoveryError::NotRetryable(
                "selected_intent_state_unavailable".to_string(),
            ),
        )
        .await);
    }
    if !plan.retryable {
        return Err(discard_unstarted_recovery_operation(
            db,
            operation.id,
            ModuleOperationRecoveryError::NotRetryable(plan.issue.to_string()),
        )
        .await);
    }
    if dispatcher.catalog().get(&plan.module_slug).is_none() {
        return Err(discard_unstarted_recovery_operation(
            db,
            operation.id,
            ModuleOperationRecoveryError::NotRetryable("unknown_module".to_string()),
        )
        .await);
    }

    let current_override_enabled =
        match read_tenant_override_enabled(db, plan.tenant_id, &plan.module_slug).await {
            Ok(current_override_enabled) => current_override_enabled,
            Err(error) => {
                return Err(mark_recovery_operation_failed(
                    db,
                    operation.id,
                    ModuleOperationRecoveryError::Persistence(error.to_string()),
                )
                .await);
            }
        };
    if current_override_enabled != plan.requested_override_enabled {
        return Err(discard_unstarted_recovery_operation(
            db,
            operation.id,
            ModuleOperationRecoveryError::StateMismatch {
                requested_override_enabled: plan.requested_override_enabled,
                current_override_enabled,
            },
        )
        .await);
    }
    let current_settings = match load_current_lifecycle_settings(
        db,
        dispatcher,
        plan.tenant_id,
        &plan.module_slug,
    )
    .await
    {
        Ok(current_settings) => current_settings,
        Err(error) => {
            return Err(mark_recovery_operation_failed(
                db,
                operation.id,
                ModuleOperationRecoveryError::Persistence(error.to_string()),
            )
            .await);
        }
    };

    if let Err(error) = record_operation_override_state(
        db,
        operation.id,
        plan.previous_override_enabled,
        plan.requested_override_enabled,
    )
    .await
    {
        let error = ModuleOperationRecoveryError::Persistence(format!("recovery-state: {error}"));
        return Err(mark_recovery_operation_failed(db, operation.id, error).await);
    }
    ModuleOperationJournal::mark_running(db, operation.id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
    // The state this hook belongs to was committed by the original
    // operation, so the retry enters the same durable shape the executor
    // uses between state commit and post-hook completion. A crash or
    // claim loss from here leaves a stale pending operation that the
    // journal reaper routes back into committed-hook recovery instead of
    // a plain failure.
    ModuleOperationJournal::mark_committed_hook_pending(db, operation.id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;

    let phase = if plan.requested_enabled {
        ModuleLifecycleHookPhase::PostEnable
    } else {
        ModuleLifecycleHookPhase::PostDisable
    };
    if let Err(error) = dispatch_lifecycle_with_claim_heartbeat(
        db,
        dispatcher,
        plan.tenant_id,
        &plan.module_slug,
        &current_settings,
        phase,
        request.expected_revision,
        request.context.idempotency_key,
    )
    .await
    {
        return Err(match error {
            ClaimHeartbeatDispatchError::Hook(message) => {
                // Typed status split: the hook outcome is the status, not a
                // message prefix. Legacy prefix parsing in
                // `ModuleOperationRecoveryPlan::from_snapshot` remains
                // read-only compatibility for pre-split rows.
                match ModuleOperationJournal::mark_committed_hook_failed(
                    db,
                    operation.id,
                    &message,
                )
                .await
                {
                    Ok(()) => ModuleOperationRecoveryError::PostHookFailed(message),
                    Err(journal_error) => ModuleOperationRecoveryError::Persistence(format!(
                        "post-hook failed: {message}; additionally, recording the \
                         committed-hook failure did not persist: {journal_error}"
                    )),
                }
            }
            // Claim loss and dispatch-persistence failures leave the
            // outcome unknown: the operation stays `CommittedHookPending`
            // for the journal reaper instead of being misrecorded as a
            // plain (pre-commit) failure.
            ClaimHeartbeatDispatchError::ClaimLost { .. } => {
                ModuleOperationRecoveryError::ClaimLost
            }
            ClaimHeartbeatDispatchError::Persistence(message) => {
                ModuleOperationRecoveryError::Persistence(message)
            }
        });
    }
    ModuleOperationJournal::mark_committed(db, operation.id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
    if plan.issue == ModuleOperationIssue::PostHookFailed {
        // The original operation's state was already committed; the hook
        // has now succeeded, so resolve it instead of leaving it in the
        // attention-needed list forever. This covers both the typed
        // `CommittedHookFailed` originals and legacy `Failed` rows whose
        // post-hook classification came from the pre-split message
        // prefix.
        let resolved = ModuleOperationJournal::resolve_post_hook_failure(db, plan.operation_id)
            .await
            .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
        if !resolved {
            return Err(ModuleOperationRecoveryError::Persistence(
                "original operation was not in a resolvable post-hook-failed state".to_string(),
            ));
        }
    }
    Ok(operation)
}

async fn discard_unstarted_recovery_operation(
    db: &DatabaseConnection,
    operation_id: uuid::Uuid,
    primary_error: ModuleOperationRecoveryError,
) -> ModuleOperationRecoveryError {
    match ModuleOperationJournal::discard_validated(db, operation_id).await {
        Ok(true) => primary_error,
        Ok(false) => ModuleOperationRecoveryError::Persistence(format!(
            "{primary_error}; additionally, the unstarted recovery operation \
             was no longer in the validated state"
        )),
        Err(discard_error) => ModuleOperationRecoveryError::Persistence(format!(
            "{primary_error}; additionally, discarding the unstarted recovery \
             operation failed: {discard_error}"
        )),
    }
}

async fn mark_recovery_operation_failed(
    db: &DatabaseConnection,
    operation_id: uuid::Uuid,
    primary_error: ModuleOperationRecoveryError,
) -> ModuleOperationRecoveryError {
    match ModuleOperationJournal::mark_failed(db, operation_id, &primary_error.to_string()).await
    {
        Ok(()) => primary_error,
        Err(journal_error) => ModuleOperationRecoveryError::Persistence(format!(
            "{primary_error}; additionally, marking the recovery operation failed \
             did not persist: {journal_error}"
        )),
    }
}

fn map_lifecycle_claim_error(
    error: StaticTenantLifecycleStoreError,
) -> ModuleOperationRecoveryError {
    match error {
        StaticTenantLifecycleStoreError::RevisionConflict {
            expected, current, ..
        } => ModuleOperationRecoveryError::RevisionConflict { expected, current },
        StaticTenantLifecycleStoreError::OperationInProgress { .. } => {
            ModuleOperationRecoveryError::OperationInProgress
        }
        error => ModuleOperationRecoveryError::Persistence(error.to_string()),
    }
}

pub(crate) async fn operation_override_state<C: ConnectionTrait>(
    db: &C,
    operation_id: uuid::Uuid,
) -> Result<Option<ModuleOperationOverrideState>, ModuleOperationStoreError> {
    let backend = SupportedBackend::new(db.get_database_backend()).map_err(store_database_error)?;
    let sql = match backend {
        SupportedBackend::Postgres => {
            "SELECT previous_override_enabled, requested_override_enabled \
             FROM module_operation_override_states WHERE operation_id = $1 LIMIT 1"
        }
        SupportedBackend::Sqlite => {
            "SELECT previous_override_enabled, requested_override_enabled \
             FROM module_operation_override_states WHERE operation_id = ?1 LIMIT 1"
        }
    };
    db.query_one_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        sql,
        vec![operation_id.into()],
    ))
    .await
    .map_err(store_database_error)?
    .map(|row| {
        Ok(ModuleOperationOverrideState {
            previous_enabled: row
                .try_get("", "previous_override_enabled")
                .map_err(store_database_error)?,
            requested_enabled: row
                .try_get("", "requested_override_enabled")
                .map_err(store_database_error)?,
        })
    })
    .transpose()
}

pub(crate) async fn record_operation_override_state<C: ConnectionTrait>(
    db: &C,
    operation_id: uuid::Uuid,
    previous_override_enabled: Option<bool>,
    requested_override_enabled: Option<bool>,
) -> Result<(), ModuleOperationStoreError> {
    let backend = SupportedBackend::new(db.get_database_backend()).map_err(store_database_error)?;
    let sql = match backend {
        SupportedBackend::Postgres => {
            "INSERT INTO module_operation_override_states \
             (operation_id, previous_override_enabled, requested_override_enabled) \
             VALUES ($1, $2, $3)"
        }
        SupportedBackend::Sqlite => {
            "INSERT INTO module_operation_override_states \
             (operation_id, previous_override_enabled, requested_override_enabled) \
             VALUES (?1, ?2, ?3)"
        }
    };
    db.execute_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        sql,
        vec![
            operation_id.into(),
            previous_override_enabled.into(),
            requested_override_enabled.into(),
        ],
    ))
    .await
    .map_err(store_database_error)?;
    Ok(())
}

pub(crate) async fn read_tenant_override_enabled<C: ConnectionTrait>(
    db: &C,
    tenant_id: uuid::Uuid,
    module_slug: &str,
) -> Result<Option<bool>, ModuleOperationStoreError> {
    let backend = SupportedBackend::new(db.get_database_backend()).map_err(store_database_error)?;
    let sql = match backend {
        SupportedBackend::Postgres => {
            "SELECT enabled FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
        }
        SupportedBackend::Sqlite => {
            "SELECT enabled FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
        }
    };
    let intent = db
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            vec![backend.uuid_value(tenant_id), module_slug.into()],
        ))
        .await
        .map_err(store_database_error)?
        .map(|row| {
            row.try_get::<Option<bool>>("", "enabled")
                .map_err(store_database_error)
        })
        .transpose()?;
    // No row and a row without recorded intent both mean "no override".
    Ok(intent.flatten())
}

pub(crate) async fn apply_tenant_override_enabled<C: ConnectionTrait>(
    db: &C,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    requested_override_enabled: Option<bool>,
) -> Result<Option<TenantModuleStateRecord>, ModuleOperationStoreError> {
    let Some(enabled) = requested_override_enabled else {
        let backend =
            SupportedBackend::new(db.get_database_backend()).map_err(store_database_error)?;
        let sql = match backend {
            // Clearing the override returns the tenant to the platform
            // default; the row and its settings survive (tri-state intent).
            SupportedBackend::Postgres => {
                "UPDATE tenant_modules SET enabled = NULL, updated_at = NOW() \
                 WHERE tenant_id = $1 AND module_slug = $2"
            }
            SupportedBackend::Sqlite => {
                "UPDATE tenant_modules SET enabled = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now') \
                 WHERE tenant_id = ?1 AND module_slug = ?2"
            }
        };
        db.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            vec![backend.uuid_value(tenant_id), module_slug.into()],
        ))
        .await
        .map_err(store_database_error)?;
        return Ok(None);
    };

    TenantModuleStateStore::persist(
        db,
        TenantModuleStateRequest {
            tenant_id,
            module_slug: module_slug.to_string(),
            enabled,
        },
    )
    .await
    .map(Some)
}

fn store_database_error(error: impl std::fmt::Display) -> ModuleOperationStoreError {
    ModuleOperationStoreError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    fn snapshot(error_message: Option<&str>) -> ModuleOperationSnapshot {
        ModuleOperationSnapshot {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            module_slug: "sample_module".to_string(),
            requested_enabled: true,
            previous_effective_enabled: false,
            status: ModuleOperationStatus::Failed,
            requested_by: Some("operator".to_string()),
            trace_id: Some("test:recovery".to_string()),
            correlation_id: Some(Uuid::new_v4().to_string()),
            idempotency_key: None,
            expected_revision: None,
            error_message: error_message.map(str::to_string),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn override_state() -> ModuleOperationOverrideState {
        ModuleOperationOverrideState {
            previous_enabled: Some(false),
            requested_enabled: Some(true),
        }
    }

    #[test]
    fn only_post_hook_failures_with_selected_state_are_retryable() {
        let post_hook = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("post-hook: timeout")),
            Some(override_state()),
        );
        assert_eq!(post_hook.issue, ModuleOperationIssue::PostHookFailed);
        assert!(post_hook.retryable);
        assert!(post_hook.override_state_recorded);
        assert_eq!(
            post_hook.recommended_action,
            ModuleOperationRecoveryAction::RetryPostHook
        );

        let legacy_post_hook =
            ModuleOperationRecoveryPlan::from_snapshot(snapshot(Some("post-hook: timeout")), None);
        assert_eq!(legacy_post_hook.issue, ModuleOperationIssue::PostHookFailed);
        assert!(!legacy_post_hook.retryable);
        assert!(!legacy_post_hook.override_state_recorded);
        assert_eq!(
            legacy_post_hook.recommended_action,
            ModuleOperationRecoveryAction::None
        );

        let pre_hook = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("pre-hook: denied")),
            Some(override_state()),
        );
        assert_eq!(pre_hook.issue, ModuleOperationIssue::PreHookFailed);
        assert!(!pre_hook.retryable);
        assert_eq!(
            pre_hook.recommended_action,
            ModuleOperationRecoveryAction::RepeatToggle
        );

        let mut orphaned_snapshot = snapshot(Some("owner did not finish within the claim lease"));
        orphaned_snapshot.status = ModuleOperationStatus::Orphaned;
        let orphaned = ModuleOperationRecoveryPlan::from_snapshot(
            orphaned_snapshot,
            Some(override_state()),
        );
        assert_eq!(orphaned.issue, ModuleOperationIssue::Orphaned);
        assert!(!orphaned.retryable);
        assert_eq!(
            orphaned.recommended_action,
            ModuleOperationRecoveryAction::RepeatToggle
        );

        let state_commit = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("state-commit: module lifecycle persistence failed")),
            Some(override_state()),
        );
        assert_eq!(state_commit.issue, ModuleOperationIssue::OtherFailed);
        assert!(!state_commit.retryable);
        assert_eq!(
            state_commit.recommended_action,
            ModuleOperationRecoveryAction::None
        );

        let recovery_state = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("recovery-state: persistence failed")),
            Some(override_state()),
        );
        assert_eq!(recovery_state.issue, ModuleOperationIssue::OtherFailed);
        assert!(!recovery_state.retryable);
        assert_eq!(
            recovery_state.recommended_action,
            ModuleOperationRecoveryAction::None
        );
    }

    #[test]
    fn owner_projects_a_redacted_recovery_view() {
        let plan = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("post-hook: timeout")),
            Some(override_state()),
        );
        let operation_id = plan.operation_id;
        let tenant_id = plan.tenant_id;
        let view = ModuleOperationRecoveryPlanView::from(plan);

        assert_eq!(view.operation_id, operation_id.to_string());
        assert_eq!(view.tenant_id, tenant_id.to_string());
        assert_eq!(view.issue, "post_hook_failed");
        assert_eq!(view.recommended_action, "retry_post_hook");
        assert!(view.correlation_id.is_some());
        assert_eq!(view.requested_by.as_deref(), Some("operator"));
        assert!(
            serde_json::to_value(view)
                .expect("recovery view serializes")
                .get("traceId")
                .is_none()
        );
    }

    #[test]
    fn retry_attempt_preserves_original_selected_predecessor_for_compensation() {
        let plan = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("post-hook: timeout")),
            Some(override_state()),
        );
        let context = ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Some(plan.tenant_id),
            trace_id: "test:retry".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };
        let request = retry_operation_request(&plan, &context, 4);

        assert_eq!(request.requested_enabled, plan.requested_enabled);
        assert_eq!(request.expected_revision, Some(4));
        assert_eq!(
            request.previous_effective_enabled,
            plan.previous_effective_enabled
        );
        assert_eq!(plan.previous_override_enabled, Some(false));
        assert_eq!(plan.requested_override_enabled, Some(true));
        let actor_id = context.actor_id.to_string();
        assert_eq!(request.requested_by.as_deref(), Some(actor_id.as_str()));
        assert_eq!(request.trace_id.as_deref(), Some(context.trace_id.as_str()));
        assert_eq!(request.correlation_id, context.correlation_id.to_string());
        assert_eq!(request.idempotency_key, Some(context.idempotency_key));
    }

    #[test]
    fn missing_override_state_is_distinct_from_inherited_predecessor() {
        let legacy =
            ModuleOperationRecoveryPlan::from_snapshot(snapshot(Some("post-hook: timeout")), None);
        let inherited = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("post-hook: timeout")),
            Some(ModuleOperationOverrideState {
                previous_enabled: None,
                requested_enabled: Some(true),
            }),
        );

        assert!(!legacy.override_state_recorded);
        assert!(inherited.override_state_recorded);
        assert_eq!(inherited.previous_override_enabled, None);
    }
}
