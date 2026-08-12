//! Shared installer foundation contracts for RusToK.
//!
//! The crate intentionally contains no HTTP, CLI, or database adapter code.
//! Those layers should call these typed contracts instead of reimplementing
//! install state, checksum, secret-redaction, and preflight policy rules.

mod deployment;
mod execution;
mod layout;
mod plan;
mod preflight;
mod receipt;
mod secrets;
#[cfg(feature = "seed-runtime")]
mod seed;
mod state;

pub use deployment::{
    DistributedDeploymentOutput, InstallDeploymentPort, InstallDistributionDeployment,
    InstallDistributionDeploymentReceipt, InstallDistributionDeploymentRequest,
    InstallRoleDeploymentObservation, distributed_deployment_request,
    execute_distributed_deployment,
};
#[cfg(feature = "host-runtime")]
pub use execution::execute_install_apply;
pub use execution::{
    InstallAdminOutcome, InstallAdminPort, InstallApplyOptions, InstallApplyOutput,
    InstallDatabasePort, InstallDatabaseReady, InstallExecutionError, InstallExecutor,
    InstallPersistencePort, InstallReceiptRecord, InstallSchemaPort, InstallSeedOutcome,
    InstallSeedPort, InstallSessionRecord, InstallVerificationOutcome, InstallVerificationPort,
};
pub use layout::{
    INSTANCE_LAYOUT_REVISION, InstanceLayout, InstanceLayoutError, InstanceLayoutMarker,
    InstanceLayoutPreparation, InstancePlacement,
};
#[cfg(feature = "host-runtime")]
pub use layout::{bind_instance_placement, prepare_instance_layout};
pub use plan::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallComposition, InstallDistributionBinding,
    InstallEnvironment, InstallPlan, InstallProfile, InstallRole, InstallRoleAssignment,
    InstallSurface, InstallTopology, InstallTopologyMode, ModuleSelection, SeedProfile,
    TenantBootstrap,
};
pub use preflight::{
    PreflightIssue, PreflightReport, PreflightSeverity, evaluate_preflight,
    evaluate_preflight_with_deployment,
};
#[cfg(feature = "host-runtime")]
pub use receipt::load_base_distribution_receipt;
pub use receipt::{
    InstallReceipt, ReceiptError, ReceiptOutcome, VerifiedInstallBaseDistributionReceipt,
    checksum_json,
};
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
