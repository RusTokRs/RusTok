use std::collections::HashSet;

use sea_orm::DatabaseConnection;
use thiserror::Error;

use crate::{
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, ModuleOperationIssue,
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecordOutcome,
    ModuleOperationRecoveryAction, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStatus,
};

/// Transport-neutral recovery view of a failed lifecycle operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOperationRecoveryPlan {
    pub operation_id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    pub requested_enabled: bool,
    pub previous_effective_enabled: bool,
    pub status: ModuleOperationStatus,
    pub issue: ModuleOperationIssue,
    pub retryable: bool,
    pub recommended_action: ModuleOperationRecoveryAction,
    pub correlation_id: Option<String>,
    pub requested_by: Option<String>,
    pub error_message: Option<String>,
}

impl ModuleOperationRecoveryPlan {
    fn from_snapshot(operation: ModuleOperationSnapshot) -> Self {
        let issue = match (operation.status, operation.error_message.as_deref()) {
            (ModuleOperationStatus::Failed, Some(message)) if message.starts_with("post-hook:") => {
                ModuleOperationIssue::PostHookFailed
            }
            (ModuleOperationStatus::Failed, Some(message))
                if message.starts_with("state-commit:") =>
            {
                ModuleOperationIssue::OtherFailed
            }
            (ModuleOperationStatus::Failed, Some(message)) if !message.is_empty() => {
                ModuleOperationIssue::PreHookFailed
            }
            (ModuleOperationStatus::Failed, _) => ModuleOperationIssue::OtherFailed,
            _ => ModuleOperationIssue::None,
        };
        let retryable = issue.retryable();
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
    pub effective_enabled_modules: HashSet<String>,
    pub current_settings: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum ModuleOperationRecoveryError {
    #[error("module operation not found")]
    OperationNotFound,
    #[error("module operation idempotency key must not be nil")]
    InvalidIdempotencyKey,
    #[error("module operation idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("module operation is not retryable: {0}")]
    NotRetryable(String),
    #[error(
        "module operation state mismatch: requested enabled={requested_enabled}, current enabled={current_enabled}"
    )]
    StateMismatch {
        requested_enabled: bool,
        current_enabled: bool,
    },
    #[error("module post-hook retry failed: {0}")]
    PostHookFailed(String),
    #[error("module operation persistence failed: {0}")]
    Persistence(String),
}

pub async fn module_operation_recovery_plan(
    db: &DatabaseConnection,
    operation_id: uuid::Uuid,
) -> Result<ModuleOperationRecoveryPlan, ModuleOperationRecoveryError> {
    ModuleOperationJournal::find(db, operation_id)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))?
        .map(ModuleOperationRecoveryPlan::from_snapshot)
        .ok_or(ModuleOperationRecoveryError::OperationNotFound)
}

pub async fn failed_module_operation_recovery_plans(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: Option<&str>,
) -> Result<Vec<ModuleOperationRecoveryPlan>, ModuleOperationRecoveryError> {
    ModuleOperationJournal::failed_for_tenant(db, tenant_id, module_slug)
        .await
        .map_err(|error| ModuleOperationRecoveryError::Persistence(error.to_string()))
        .map(|operations| {
            operations
                .into_iter()
                .map(ModuleOperationRecoveryPlan::from_snapshot)
                .collect()
        })
}

fn retry_operation_request(
    plan: &ModuleOperationRecoveryPlan,
    requested_by: Option<String>,
    idempotency_key: uuid::Uuid,
) -> ModuleOperationRequest {
    ModuleOperationRequest {
        tenant_id: plan.tenant_id,
        module_slug: plan.module_slug.clone(),
        requested_enabled: plan.requested_enabled,
        previous_effective_enabled: plan.previous_effective_enabled,
        requested_by,
        correlation_id: plan.operation_id.to_string(),
        idempotency_key: Some(idempotency_key),
    }
}

pub async fn retry_failed_post_hook_operation(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    request: ModulePostHookRetryRequest,
) -> Result<ModuleOperationRecord, ModuleOperationRecoveryError> {
    if request.idempotency_key.is_nil() {
        return Err(ModuleOperationRecoveryError::InvalidIdempotencyKey);
    }
    let plan = module_operation_recovery_plan(db, request.operation_id).await?;
    let journal_request =
        retry_operation_request(&plan, request.requested_by.clone(), request.idempotency_key);
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
    let current_enabled = request
        .effective_enabled_modules
        .contains(&plan.module_slug);
    if current_enabled != plan.requested_enabled {
        return Err(ModuleOperationRecoveryError::StateMismatch {
            requested_enabled: plan.requested_enabled,
            current_enabled,
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
        ModuleOperationRecordOutcome::Recorded(operation) => operation,
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
            error_message: error_message.map(str::to_string),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn only_post_hook_failures_are_retryable() {
        let post_hook =
            ModuleOperationRecoveryPlan::from_snapshot(snapshot(Some("post-hook: timeout")));
        assert_eq!(post_hook.issue, ModuleOperationIssue::PostHookFailed);
        assert!(post_hook.retryable);
        assert_eq!(
            post_hook.recommended_action,
            ModuleOperationRecoveryAction::RetryPostHook
        );

        let pre_hook =
            ModuleOperationRecoveryPlan::from_snapshot(snapshot(Some("pre-hook: denied")));
        assert_eq!(pre_hook.issue, ModuleOperationIssue::PreHookFailed);
        assert!(!pre_hook.retryable);
        assert_eq!(
            pre_hook.recommended_action,
            ModuleOperationRecoveryAction::RepeatToggle
        );

        let state_commit = ModuleOperationRecoveryPlan::from_snapshot(snapshot(Some(
            "state-commit: module lifecycle persistence failed",
        )));
        assert_eq!(state_commit.issue, ModuleOperationIssue::OtherFailed);
        assert!(!state_commit.retryable);
        assert_eq!(
            state_commit.recommended_action,
            ModuleOperationRecoveryAction::None
        );
    }

    #[test]
    fn retry_attempt_preserves_original_previous_state_for_compensation() {
        let plan = ModuleOperationRecoveryPlan::from_snapshot(snapshot(Some("post-hook: timeout")));
        let request =
            retry_operation_request(&plan, Some("retry-operator".to_string()), Uuid::new_v4());

        assert_eq!(request.requested_enabled, plan.requested_enabled);
        assert_eq!(
            request.previous_effective_enabled,
            plan.previous_effective_enabled
        );
        assert_ne!(
            request.previous_effective_enabled,
            request.requested_enabled
        );
        assert_eq!(request.correlation_id, plan.operation_id.to_string());
    }
}
