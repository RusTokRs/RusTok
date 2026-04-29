//! Shared installer foundation contracts for RusToK.
//!
//! The crate intentionally contains no HTTP, CLI, or database adapter code.
//! Those layers should call these typed contracts instead of reimplementing
//! install state, checksum, secret-redaction, and preflight policy rules.

mod plan;
mod preflight;
mod receipt;
mod secrets;
mod state;

pub use plan::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan,
    InstallProfile, ModuleSelection, SeedProfile, TenantBootstrap,
};
pub use preflight::{evaluate_preflight, PreflightIssue, PreflightReport, PreflightSeverity};
pub use receipt::{checksum_json, InstallReceipt, ReceiptError, ReceiptOutcome};
pub use secrets::{redact_install_plan, redact_secret, SecretMode, SecretRef, SecretValue};
pub use state::{InstallState, InstallStep, StateTransitionError};
