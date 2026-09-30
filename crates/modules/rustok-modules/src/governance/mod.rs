//! Registry module governance contracts, state machines, and lifecycle transitions.

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
pub mod marketplace;
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

pub(crate) use helpers::*;
pub(crate) use mapping::*;
pub(crate) use receipts::*;
pub(crate) use receipts_reviews::*;
pub(crate) use validation_evidence::*;
pub(crate) use validation_work_items::*;
pub(crate) use publication_verification::*;

#[cfg(test)]
mod tests;
