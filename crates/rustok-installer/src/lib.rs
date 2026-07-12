//! Shared installer foundation contracts for RusToK.
//!
//! The crate intentionally contains no HTTP, CLI, or database adapter code.
//! Those layers should call these typed contracts instead of reimplementing
//! install state, checksum, secret-redaction, and preflight policy rules.

mod execution;
mod plan;
mod preflight;
mod receipt;
mod secrets;
mod seed;
mod state;

pub use execution::{
    InstallApplyOptions, InstallApplyOutput, InstallExecutionError, InstallExecutor,
};
pub use plan::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan,
    InstallProfile, ModuleSelection, SeedProfile, TenantBootstrap,
};
pub use preflight::{evaluate_preflight, PreflightIssue, PreflightReport, PreflightSeverity};
pub use receipt::{checksum_json, InstallReceipt, ReceiptError, ReceiptOutcome};
pub use secrets::{redact_install_plan, redact_secret, SecretMode, SecretRef, SecretValue};
pub use seed::{
    execute_seed_profile, SeedExecutionError, SeedExecutionOutcome, SeedExecutionRequest,
    SeedIdentityPort, SeedModulePort, SeedRolePort, SeedTenant, SeedTenantPort, SeedTenantRequest,
    SeedUser, SeedUserRequest,
};
pub use state::{InstallState, InstallStep, StateTransitionError};
