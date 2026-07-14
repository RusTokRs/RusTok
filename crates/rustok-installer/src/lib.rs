//! Shared installer foundation contracts for RusToK.
//!
//! The crate intentionally contains no HTTP, CLI, or database adapter code.
//! Those layers should call these typed contracts instead of reimplementing
//! install state, checksum, secret-redaction, and preflight policy rules.

mod deployment;
mod execution;
mod plan;
mod preflight;
mod receipt;
mod secrets;
mod seed;
mod state;

pub use deployment::{
    distributed_deployment_requests, execute_distributed_role_deployments,
    DistributedDeploymentOutput, InstallDeploymentPort, InstallRoleDeployment,
    InstallRoleDeploymentReceipt, InstallRoleDeploymentRequest,
};
pub use execution::{
    execute_install_apply, InstallAdminOutcome, InstallAdminPort, InstallApplyOptions,
    InstallApplyOutput, InstallDatabasePort, InstallDatabaseReady, InstallExecutionError,
    InstallExecutor, InstallPersistencePort, InstallReceiptRecord, InstallSchemaPort,
    InstallSeedOutcome, InstallSeedPort, InstallSessionRecord, InstallVerificationOutcome,
    InstallVerificationPort,
};
pub use plan::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallComposition, InstallEnvironment,
    InstallPlan, InstallProfile, InstallRole, InstallRoleAssignment, InstallSurface,
    InstallTopology, InstallTopologyMode, ModuleSelection, SeedProfile, TenantBootstrap,
};
pub use preflight::{
    evaluate_preflight, evaluate_preflight_with_deployment, PreflightIssue, PreflightReport,
    PreflightSeverity,
};
pub use receipt::{checksum_json, InstallReceipt, ReceiptError, ReceiptOutcome};
pub use secrets::{
    redact_install_plan, redact_secret, resolve_local_secret_value, SecretMode, SecretRef,
    SecretResolutionError, SecretValue,
};
pub use seed::{
    execute_seed_profile, SeedExecutionError, SeedExecutionOutcome, SeedExecutionRequest,
    SeedIdentityPort, SeedModulePort, SeedPrincipalPort, SeedRolePort, SeedTenant, SeedTenantPort,
    SeedTenantRequest, SeedUser, SeedUserRequest,
};
pub use state::{InstallState, InstallStep, StateTransitionError};
