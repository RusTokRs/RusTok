use rustok_modules::{
    ModuleControlPlane, ModuleGovernanceError, ModuleGovernanceErrorCategory,
    ModuleRemoteValidationHeartbeatCommand, ModuleRemoteValidationStageTransition,
    ModuleRemoteValidationTerminalCommand, ModuleRemoteValidationTerminalOutcome,
};
use sea_orm::DatabaseConnection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteTerminalOutcome {
    Passed,
    Failed,
}

/// Owner-issued remote-transition failure facts. This adapter preserves the
/// canonical category and code rather than maintaining a partial local copy of
/// the governance error taxonomy.
#[derive(Debug, thiserror::Error)]
#[error("{detail}")]
pub struct RegistryRemoteTransitionError {
    pub category: ModuleGovernanceErrorCategory,
    pub code: &'static str,
    pub detail: String,
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
    RegistryRemoteTransitionError {
        category: error.category(),
        code: error.code(),
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_transition_preserves_owner_error_contract() {
        let error = map_owner_remote_lease_error(
            ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch,
        );

        assert_eq!(
            error.category,
            ModuleGovernanceErrorCategory::PermissionDenied
        );
        assert_eq!(error.code, "module_governance_permission_denied");
    }
}
