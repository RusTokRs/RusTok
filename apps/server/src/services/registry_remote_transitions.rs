use rustok_modules::{
    ModuleControlPlane, ModuleGovernanceError, ModuleRemoteValidationHeartbeatCommand,
    ModuleRemoteValidationStageTransition, ModuleRemoteValidationTerminalCommand,
    ModuleRemoteValidationTerminalOutcome,
};
use sea_orm::DatabaseConnection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteTerminalOutcome {
    Passed,
    Failed,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryRemoteTransitionError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Internal(String),
}

pub async fn heartbeat_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    claim_id: &str,
    runner_id: &str,
    lease_ttl_ms: u64,
) -> Result<ModuleRemoteValidationStageTransition, RegistryRemoteTransitionError> {
    ModuleControlPlane::new(db.clone())
        .publication()
        .heartbeat_remote_validation_stage(ModuleRemoteValidationHeartbeatCommand {
            claim_id: claim_id.to_string(),
            runner_id: runner_id.to_string(),
            lease_ttl_ms,
        })
        .await
        .map_err(map_owner_remote_lease_error)
}

pub async fn finish_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    claim_id: &str,
    runner_id: &str,
    outcome: RemoteTerminalOutcome,
    detail: Option<&str>,
    reason_code: Option<&str>,
) -> Result<ModuleRemoteValidationStageTransition, RegistryRemoteTransitionError> {
    ModuleControlPlane::new(db.clone())
        .publication()
        .complete_remote_validation_stage(ModuleRemoteValidationTerminalCommand {
            claim_id: claim_id.to_string(),
            runner_id: runner_id.to_string(),
            outcome: match outcome {
                RemoteTerminalOutcome::Passed => ModuleRemoteValidationTerminalOutcome::Passed,
                RemoteTerminalOutcome::Failed => ModuleRemoteValidationTerminalOutcome::Failed,
            },
            detail: detail.map(ToString::to_string),
            reason_code: reason_code.map(|value| value.trim().to_ascii_lowercase()),
        })
        .await
        .map_err(map_owner_remote_lease_error)
}

fn map_owner_remote_lease_error(error: ModuleGovernanceError) -> RegistryRemoteTransitionError {
    match error {
        ModuleGovernanceError::InvalidRemoteValidationLeaseCommand
        | ModuleGovernanceError::InvalidValidationStageReasonCode(_) => {
            RegistryRemoteTransitionError::Invalid(error.to_string())
        }
        ModuleGovernanceError::RemoteValidationLeaseNotFound => {
            RegistryRemoteTransitionError::NotFound(error.to_string())
        }
        ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch => {
            RegistryRemoteTransitionError::Forbidden(error.to_string())
        }
        ModuleGovernanceError::RemoteValidationLeaseNotRunning(_)
        | ModuleGovernanceError::RemoteValidationLeaseExpired => {
            RegistryRemoteTransitionError::Conflict(error.to_string())
        }
        _ => RegistryRemoteTransitionError::Internal(error.to_string()),
    }
}
