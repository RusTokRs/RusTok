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
                | (AdminProvisioned, Deploying)
                | (Deploying, Verified)
                | (Verified, Completed)
                | (Draft, FreshInstallCleaned)
                | (PreflightPassed, FreshInstallCleaned)
                | (ConfigPrepared, FreshInstallCleaned)
                | (DatabaseReady, FreshInstallCleaned)
                | (Draft, RecoveryRequired)
                | (PreflightPassed, RecoveryRequired)
                | (ConfigPrepared, RecoveryRequired)
                | (DatabaseReady, RecoveryRequired)
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
            .transition_to(InstallState::Deploying)
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
    fn ambiguous_effects_require_recovery_from_every_nonterminal_stage() {
        for state in [
            InstallState::Draft,
            InstallState::PreflightPassed,
            InstallState::ConfigPrepared,
            InstallState::DatabaseReady,
            InstallState::SchemaApplied,
            InstallState::SeedApplied,
            InstallState::AdminProvisioned,
            InstallState::Deploying,
            InstallState::Verified,
        ] {
            assert!(
                state.can_transition_to(InstallState::RecoveryRequired),
                "{state:?} must be quarantinable after an ambiguous effect"
            );
        }
        assert!(!InstallState::Completed.can_transition_to(InstallState::RecoveryRequired));
        assert!(!InstallState::FreshInstallCleaned.can_transition_to(InstallState::RecoveryRequired));
    }

    #[test]
    fn failed_is_legacy_only_and_cannot_release_a_new_attempt() {
        for state in [
            InstallState::Draft,
            InstallState::SchemaApplied,
            InstallState::Deploying,
            InstallState::Completed,
            InstallState::RecoveryRequired,
        ] {
            assert!(
                !state.can_transition_to(InstallState::Failed),
                "{state:?} must not be newly rewritten as failed"
            );
        }
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
