use rustok_modules::{
    ModuleControlPlane, ModuleGovernanceError, ModuleRemoteValidationHeartbeatCommand,
    ModuleRemoteValidationTerminalCommand, ModuleRemoteValidationTerminalOutcome,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::models::registry_validation_stage;

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
) -> Result<registry_validation_stage::Model, RegistryRemoteTransitionError> {
    ModuleControlPlane::new(db.clone())
        .publication()
        .heartbeat_remote_validation_stage(ModuleRemoteValidationHeartbeatCommand {
            claim_id: claim_id.to_string(),
            runner_id: runner_id.to_string(),
            lease_ttl_ms,
        })
        .await
        .map_err(map_owner_remote_lease_error)?;

    load_stage_by_claim(db, claim_id, "Heartbeat").await
}

pub async fn finish_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    claim_id: &str,
    runner_id: &str,
    outcome: RemoteTerminalOutcome,
    detail: Option<&str>,
    reason_code: Option<&str>,
) -> Result<registry_validation_stage::Model, RegistryRemoteTransitionError> {
    let stage_id = ModuleControlPlane::new(db.clone())
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
        .map_err(map_owner_remote_lease_error)?;

    registry_validation_stage::Entity::find_by_id(stage_id)
        .one(db)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            RegistryRemoteTransitionError::Internal(
                "Terminal update succeeded but registry validation stage disappeared".to_string(),
            )
        })
}

async fn load_stage_by_claim(
    db: &DatabaseConnection,
    claim_id: &str,
    operation: &str,
) -> Result<registry_validation_stage::Model, RegistryRemoteTransitionError> {
    registry_validation_stage::Entity::find()
        .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
        .one(db)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            RegistryRemoteTransitionError::Internal(format!(
                "{operation} succeeded but registry validation stage disappeared"
            ))
        })
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

fn internal(error: impl std::fmt::Display) -> RegistryRemoteTransitionError {
    RegistryRemoteTransitionError::Internal(error.to_string())
}
