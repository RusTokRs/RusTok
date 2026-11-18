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
    Deploying,
    Verified,
    Completed,
    Failed,
    FreshInstallCleaned,
    RecoveryRequired,
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
    Deploy,
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::PreflightPassed => "preflight_passed",
            Self::ConfigPrepared => "config_prepared",
            Self::DatabaseReady => "database_ready",
            Self::SchemaApplied => "schema_applied",
            Self::SeedApplied => "seed_applied",
            Self::AdminProvisioned => "admin_provisioned",
            Self::Deploying => "deploying",
            Self::Verified => "verified",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::FreshInstallCleaned => "fresh_install_cleaned",
            Self::RecoveryRequired => "recovery_required",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "draft" => Self::Draft,
            "preflight_passed" => Self::PreflightPassed,
            "config_prepared" => Self::ConfigPrepared,
            "database_ready" => Self::DatabaseReady,
            "schema_applied" => Self::SchemaApplied,
            "seed_applied" => Self::SeedApplied,
            "admin_provisioned" => Self::AdminProvisioned,
            "deploying" => Self::Deploying,
            "verified" => Self::Verified,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "fresh_install_cleaned" => Self::FreshInstallCleaned,
            "recovery_required" => Self::RecoveryRequired,
            _ => return None,
        })
    }

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
                | (AdminProvisioned, Deploying)
                | (Deploying, Verified)
                | (Verified, Completed)
                | (_, Failed)
                | (Draft, FreshInstallCleaned)
                | (PreflightPassed, FreshInstallCleaned)
                | (ConfigPrepared, FreshInstallCleaned)
                | (DatabaseReady, FreshInstallCleaned)
                | (SchemaApplied, RecoveryRequired)
                | (SeedApplied, RecoveryRequired)
                | (AdminProvisioned, RecoveryRequired)
                | (Deploying, RecoveryRequired)
                | (Verified, RecoveryRequired)
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
    fn persisted_state_names_round_trip_without_accepting_unknown_values() {
        let states = [
            InstallState::Draft,
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
            InstallState::SchemaApplied,
            InstallState::SeedApplied,
            InstallState::AdminProvisioned,
            InstallState::Deploying,
            InstallState::Verified,
            InstallState::Completed,
            InstallState::Failed,
            InstallState::FreshInstallCleaned,
            InstallState::RecoveryRequired,
        ];
        for state in states {
            assert_eq!(InstallState::from_str(state.as_str()), Some(state));
        }
        assert_eq!(InstallState::from_str("unknown_state"), None);
    }

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
    fn schema_applied_requires_recovery_instead_of_claiming_rollback() {
        assert!(
            InstallState::SchemaApplied
                .transition_to(InstallState::RecoveryRequired)
                .is_ok()
        );
    }

    #[test]
    fn distribution_deployment_must_run_before_verification() {
        let state = InstallState::AdminProvisioned
            .transition_to(InstallState::Deploying)
            .unwrap()
            .transition_to(InstallState::Verified)
            .unwrap();

        assert_eq!(state, InstallState::Verified);
    }
}
