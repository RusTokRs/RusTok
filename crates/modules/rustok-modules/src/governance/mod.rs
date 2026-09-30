//! Registry module governance contracts, state machines, and lifecycle transitions.

pub use crate::{ControlPlaneInfrastructure, ModuleCommandContext};

pub mod actions_ownership;
pub mod actions_review;
pub mod admissions;
pub mod commands;
pub mod constants;
pub mod error;
pub mod evidence;
pub mod helpers;
pub mod mapping;
pub mod market_projections;
pub mod projections;
pub mod publication;
pub mod publication_verification;
pub mod receipts;
pub mod receipts_reviews;
pub mod remote_runners;
pub mod snapshots;
pub mod staging;
pub mod staging_alloy;
pub mod staging_external;
pub mod types;
pub mod upload;
pub mod validation_evidence;
pub mod validation_jobs;
pub mod validation_results;
pub mod validation_stages;
pub mod validation_work_items;

pub use constants::*;
pub use error::*;
pub use snapshots::*;
pub use types::*;
pub use projections::SeaOrmModuleGovernanceService;

#[cfg(test)]
mod tests;
