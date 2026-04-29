use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallState {
    Draft,
    PreflightPassed,
    ConfigPrepared,
    DatabaseReady,
    SchemaApplied,
    SeedApplied,
    AdminProvisioned,
    Verified,
    Completed,
    Failed,
    RolledBackFreshInstall,
    RestoreRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStep {
    Preflight,
    Config,
    Database,
    Migrate,
    Seed,
    Admin,
    Verify,
    Finalize,
    Rollback,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid installer state transition from {from:?} to {to:?}")]
pub struct StateTransitionError {
    pub from: InstallState,
    pub to: InstallState,
}

impl InstallState {
    pub fn can_transition_to(self, to: InstallState) -> bool {
        use InstallState::*;

        matches!(
            (self, to),
            (Draft, PreflightPassed)
                | (PreflightPassed, ConfigPrepared)
                | (ConfigPrepared, DatabaseReady)
                | (DatabaseReady, SchemaApplied)
                | (SchemaApplied, SeedApplied)
                | (SeedApplied, AdminProvisioned)
                | (AdminProvisioned, Verified)
                | (Verified, Completed)
                | (_, Failed)
                | (Draft, RolledBackFreshInstall)
                | (PreflightPassed, RolledBackFreshInstall)
                | (ConfigPrepared, RolledBackFreshInstall)
                | (DatabaseReady, RolledBackFreshInstall)
                | (SchemaApplied, RestoreRequired)
                | (SeedApplied, RestoreRequired)
                | (AdminProvisioned, RestoreRequired)
                | (Verified, RestoreRequired)
        )
    }

    pub fn transition_to(self, to: InstallState) -> Result<InstallState, StateTransitionError> {
        if self.can_transition_to(to) {
            Ok(to)
        } else {
            Err(StateTransitionError { from: self, to })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions_are_allowed() {
        let state = InstallState::Draft
            .transition_to(InstallState::PreflightPassed)
            .unwrap()
            .transition_to(InstallState::ConfigPrepared)
            .unwrap()
            .transition_to(InstallState::DatabaseReady)
            .unwrap()
            .transition_to(InstallState::SchemaApplied)
            .unwrap()
            .transition_to(InstallState::SeedApplied)
            .unwrap()
            .transition_to(InstallState::AdminProvisioned)
            .unwrap()
            .transition_to(InstallState::Verified)
            .unwrap()
            .transition_to(InstallState::Completed)
            .unwrap();

        assert_eq!(state, InstallState::Completed);
    }

    #[test]
    fn cannot_skip_schema_step() {
        let error = InstallState::DatabaseReady
            .transition_to(InstallState::SeedApplied)
            .unwrap_err();

        assert_eq!(error.from, InstallState::DatabaseReady);
        assert_eq!(error.to, InstallState::SeedApplied);
    }

    #[test]
    fn schema_applied_rolls_forward_to_restore_required() {
        assert!(InstallState::SchemaApplied
            .transition_to(InstallState::RestoreRequired)
            .is_ok());
    }
}
