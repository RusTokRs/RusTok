//! Registry module governance contracts, state machines, and lifecycle transitions.

pub use crate::{ControlPlaneInfrastructure, ModuleCommandContext};

pub mod constants;
pub mod error;
pub mod types;
pub mod snapshots;
pub mod commands;
pub mod helpers;
pub mod receipts;
pub mod receipts_reviews;
pub mod mapping;
pub mod projections;
pub mod market_projections;
pub mod upload;
pub mod staging;
pub mod staging_external;
pub mod staging_alloy;
pub mod evidence;
pub mod admissions;
pub mod actions_ownership;
pub mod actions_review;
pub mod validation_stages;
pub mod validation_evidence;
pub mod validation_jobs;
pub mod validation_results;
pub mod validation_work_items;
pub mod remote_runners;
pub mod publication_verification;
pub mod publication;

pub use constants::*;
pub use error::*;
pub use types::*;
pub use snapshots::*;
pub use commands::*;
pub use projections::SeaOrmModuleGovernanceService;

pub(crate) use actions_ownership::*;
pub(crate) use actions_review::*;
pub(crate) use admissions::*;
pub(crate) use evidence::*;
pub(crate) use helpers::*;
pub(crate) use mapping::*;
pub(crate) use market_projections::*;
pub(crate) use projections::*;
pub(crate) use publication::*;
pub(crate) use publication_verification::*;
pub(crate) use receipts::*;
pub(crate) use receipts_reviews::*;
pub(crate) use remote_runners::*;
pub(crate) use staging::*;
pub(crate) use staging_alloy::*;
pub(crate) use staging_external::*;
pub(crate) use upload::*;
pub(crate) use validation_evidence::*;
pub(crate) use validation_jobs::*;
pub(crate) use validation_results::*;
pub(crate) use validation_stages::*;
pub(crate) use validation_work_items::*;

#[cfg(test)]
mod tests;
