use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use thiserror::Error;

use crate::{
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, ModuleOperationIssue,
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecordOutcome,
    ModuleOperationRecoveryAction, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStatus, ModuleOperationStoreError, TenantModuleStateRecord,
    TenantModuleStateRequest, TenantModuleStateStore,
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
    pub requested_by: Option<String>,
    pub error_message: Option<String>,
}

impl ModuleOperationRecoveryPlan {
    fn from_snapshot(
        operation: ModuleOperationSnapshot,
        override_state: Option<ModuleOperationOverrideState>,
    ) -> Self {
        let issue = match (operation.status, operation.error_message.as_deref()) {
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
            _ => ModuleOperationIssue::None,
        };
        let override_state_recorded = override_state.is_some();
        let retryable = issue.retryable() && override_state_recorded;
        let recommended_action = if retryable {
            ModuleOperationRecoveryAction::RetryPostHook
        } else if issue == ModuleOperationIssue::PreHookFailed {
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
            requested_by: operation.requested_by,
            error_message: operation.error_message,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModulePostHookRetryRequest {
    pub operation_id: uuid::Uuid,
    pub requested_by: Option<String>,
    pub idempotency_key: uuid::Uuid,
    /// Revision reviewed by the actor before retrying a static lifecycle
    /// operation. Dynamic artifact recovery deliberately leaves this absent.
    pub expected_revision: Option<u64>,
    /// Exact persisted tenant override after the original state commit.
    /// `None` means that no explicit tenant override row exists.
    pub current_override_enabled: Option<bool>,
    pub current_settings: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum ModuleOperationRecoveryError {
    #[error("module operation not found")]
    OperationNotFound,
    #[error("module recovery command identity must not be nil")]
    InvalidCommandIdentity,
    #[error("module operation idempotency key must not be nil")]
    InvalidIdempotencyKey,
    #[error("module operation idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("module lifecycle revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("module lifecycle operation is already active")]
    OperationInProgress,
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

fn retry_operation_request(
    plan: &ModuleOperationRecoveryPlan,
    requested_by: Option<String>,
    idempotency_key: uuid::Uuid,
    expected_revision: Option<u64>,
) -> ModuleOperationRequest {
    ModuleOperationRequest {
        tenant_id: plan.tenant_id,
        module_slug: plan.module_slug.clone(),
        requested_enabled: plan.requested_enabled,
        previous_effective_enabled: plan.previous_effective_enabled,
        requested_by,
        correlation_id: plan.operation_id.to_string(),
        idempotency_key: Some(idempotency_key),
        expected_revision,
    }
}

pub(crate) async fn retry_failed_post_hook_operation(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    request: ModulePostHookRetryRequest,
) -> Result<ModuleOperationRecord, ModuleOperationRecoveryError> {
    if request.idempotency_key.is_nil() {
        return Err(ModuleOperationRecoveryError::InvalidIdempotencyKey);
    }
    let plan = module_operation_recovery_plan(db, request.operation_id).await?;
    let journal_request = retry_operation_request(
        &plan,
        request.requested_by.clone(),
        request.idempotency_key,
        request.expected_revision,
    );
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
    if request.current_override_enabled != plan.requested_override_enabled {
        return Err(ModuleOperationRecoveryError::StateMismatch {
            requested_override_enabled: plan.requested_override_enabled,
            current_override_enabled: request.current_override_enabled,
        });
    }

    let operation = match ModuleOperationJournal::record_idempotent(db, journal_request)
        .await
        .map_err(|error| match error {
            crate::ModuleOperationStoreError::IdempotencyConflict => {
                ModuleOperationRecoveryError::IdempotencyConflict
            }
            error => ModuleOperationRecoveryError::Persistence(error.to_string()),
        })? {
        ModuleOperationRecordOutcome::Recorded(operation) => {
            if let Err(error) = record_operation_override_state(
                db,
                operation.id,
                plan.previous_override_enabled,
                plan.requested_override_enabled,
            )
            .await
            {
                let message = format!("recovery-state: {error}");
                let _ = ModuleOperationJournal::mark_failed(db, operation.id, &message).await;
                return Err(ModuleOperationRecoveryError::Persistence(error.to_string()));
            }
            operation
        }
        ModuleOperationRecordOutcome::Replayed(operation) => return Ok(operation),
    };
    ModuleOperationJournal::mark_running(db, operation.id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;

    let phase = if plan.requested_enabled {
        ModuleLifecycleHookPhase::PostEnable
    } else {
        ModuleLifecycleHookPhase::PostDisable
    };
    if let Err(error) = dispatcher
        .dispatch_lifecycle(
            db,
            plan.tenant_id,
            &plan.module_slug,
            &request.current_settings,
            phase,
        )
        .await
    {
        let message = error.to_string();
        ModuleOperationJournal::mark_failed(db, operation.id, &format!("post-hook: {message}"))
            .await
            .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
        return Err(ModuleOperationRecoveryError::PostHookFailed(message));
    }
    ModuleOperationJournal::mark_committed(db, operation.id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?;
    Ok(operation)
}

pub(crate) async fn operation_override_state<C: ConnectionTrait>(
    db: &C,
    operation_id: uuid::Uuid,
) -> Result<Option<ModuleOperationOverrideState>, ModuleOperationStoreError> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => {
            "SELECT previous_override_enabled, requested_override_enabled \
             FROM module_operation_override_states WHERE operation_id = $1 LIMIT 1"
        }
        _ => {
            "SELECT previous_override_enabled, requested_override_enabled \
             FROM module_operation_override_states WHERE operation_id = ?1 LIMIT 1"
        }
    };
    db.query_one(Statement::from_sql_and_values(
        backend,
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
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => {
            "INSERT INTO module_operation_override_states \
             (operation_id, previous_override_enabled, requested_override_enabled) \
             VALUES ($1, $2, $3)"
        }
        _ => {
            "INSERT INTO module_operation_override_states \
             (operation_id, previous_override_enabled, requested_override_enabled) \
             VALUES (?1, ?2, ?3)"
        }
    };
    db.execute(Statement::from_sql_and_values(
        backend,
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
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => {
            "SELECT enabled FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
        }
        _ => "SELECT enabled FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1",
    };
    db.query_one(Statement::from_sql_and_values(
        backend,
        sql,
        vec![tenant_id.into(), module_slug.into()],
    ))
    .await
    .map_err(store_database_error)?
    .map(|row| row.try_get("", "enabled").map_err(store_database_error))
    .transpose()
}

pub(crate) async fn apply_tenant_override_enabled<C: ConnectionTrait>(
    db: &C,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    requested_override_enabled: Option<bool>,
) -> Result<Option<TenantModuleStateRecord>, ModuleOperationStoreError> {
    let Some(enabled) = requested_override_enabled else {
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "DELETE FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2"
            }
            _ => "DELETE FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2",
        };
        db.execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), module_slug.into()],
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
            correlation_id: Some(Uuid::new_v4().to_string()),
            idempotency_key: None,
            expected_revision: None,
            error_message: error_message.map(str::to_string),
            created_at: Utc::now(),
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
    fn retry_attempt_preserves_original_selected_predecessor_for_compensation() {
        let plan = ModuleOperationRecoveryPlan::from_snapshot(
            snapshot(Some("post-hook: timeout")),
            Some(override_state()),
        );
        let request = retry_operation_request(
            &plan,
            Some("retry-operator".to_string()),
            Uuid::new_v4(),
            Some(4),
        );

        assert_eq!(request.requested_enabled, plan.requested_enabled);
        assert_eq!(request.expected_revision, Some(4));
        assert_eq!(
            request.previous_effective_enabled,
            plan.previous_effective_enabled
        );
        assert_eq!(plan.previous_override_enabled, Some(false));
        assert_eq!(plan.requested_override_enabled, Some(true));
        assert_eq!(request.correlation_id, plan.operation_id.to_string());
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
