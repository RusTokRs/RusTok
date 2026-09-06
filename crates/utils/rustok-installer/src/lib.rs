//! Shared installer foundation contracts for RusToK.
//!
//! The crate intentionally contains no HTTP, CLI, or database adapter code.
//! Those layers should call these typed contracts instead of reimplementing
//! install state, checksum, secret-redaction, and preflight policy rules.

#[cfg(feature = "host-runtime")]
mod deployment;
#[cfg(feature = "host-runtime")]
mod execution;
mod plan;
mod preflight;
mod receipt;
mod secrets;
#[cfg(feature = "seed-runtime")]
mod seed;
mod state;

#[cfg(feature = "host-runtime")]
pub use deployment::{
    DistributionDeploymentOutput, InstallDeploymentPort, InstallDistributionDeployment,
    InstallDistributionDeploymentReceipt, InstallDistributionDeploymentRequest,
    InstallRoleDeploymentObservation, distribution_deployment_request,
    execute_distribution_deployment,
};
#[cfg(feature = "host-runtime")]
pub use execution::execute_install_apply;
#[cfg(feature = "host-runtime")]
pub use execution::{
    InstallAdminOutcome, InstallAdminPort, InstallApplyOptions, InstallApplyOutput,
    InstallBootstrapPort, InstallDatabasePort, InstallDatabaseReady, InstallExecutionError,
    InstallExecutor, InstallPersistencePort, InstallReceiptRecord, InstallSchemaPort,
    InstallSeedOutcome, InstallSeedPort, InstallSessionRecord, InstallVerificationOutcome,
    InstallVerificationPort,
};
pub use plan::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallComposition, InstallDistributionBinding,
    InstallEnvironment, InstallPlan, InstallProfile, InstallRole, InstallRoleAssignment,
    InstallSurface, InstallTopology, InstallTopologyMode, ModuleSelection, SeedProfile,
    TenantBootstrap,
};
#[cfg(not(feature = "host-runtime"))]
pub use plan::{InstallDistributionRole, InstallDistributionRoleArtifact};
pub use preflight::{
    PreflightIssue, PreflightReport, PreflightSeverity, evaluate_preflight,
    evaluate_preflight_with_deployment,
};
#[cfg(feature = "host-runtime")]
pub use receipt::VerifiedInstallBaseDistributionReceipt;
#[cfg(feature = "host-runtime")]
pub use receipt::load_base_distribution_receipt;
pub use receipt::{InstallReceipt, ReceiptError, ReceiptOutcome, checksum_json};
#[cfg(feature = "host-runtime")]
pub use rustok_runtime::{
    INSTANCE_LAYOUT_REVISION, InstanceLayout, InstanceLayoutError, InstanceLayoutMarker,
    InstanceLayoutPreparation, InstancePlacement,
};
#[cfg(feature = "host-runtime")]
pub use rustok_runtime::{bind_instance_placement, prepare_instance_layout};

#[cfg(not(feature = "host-runtime"))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstancePlacement {
    pub instance_id: uuid::Uuid,
    pub root: String,
}

#[cfg(not(feature = "host-runtime"))]
impl InstancePlacement {
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            instance_id: uuid::Uuid::new_v4(),
            root: root.into(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.root.trim().is_empty() {
            return Err("instance root is required".to_string());
        }
        if self.root.contains('\0') {
            return Err("instance root contains a NUL character".to_string());
        }
        Ok(())
    }
}

pub use secrets::{
    SecretMode, SecretRef, SecretResolutionError, SecretValue, redact_install_plan, redact_secret,
    resolve_local_secret_value,
};
#[cfg(feature = "seed-runtime")]
pub use seed::{
    SeedExecutionError, SeedExecutionOutcome, SeedExecutionRequest, SeedIdentityPort,
    SeedModulePort, SeedPrincipalPort, SeedRolePort, SeedTenant, SeedTenantPort, SeedTenantRequest,
    SeedUser, SeedUserRequest, execute_seed_profile,
};
pub use state::{InstallState, InstallStep, StateTransitionError};
