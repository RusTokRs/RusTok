pub mod model;
pub mod provider;

pub use model::*;
pub use provider::*;

pub mod api {
    pub use crate::model::*;
    pub use crate::provider::*;
}

#[cfg(feature = "server")]
pub mod entities;
#[cfg(all(feature = "server", feature = "graphql"))]
pub mod graphql;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
mod reconciliation;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "server")]
mod target_lifecycle;

#[cfg(feature = "server")]
pub use reconciliation::{
    MAX_REACTION_RECONCILIATION_ACTOR_STATES, MAX_REACTION_RECONCILIATION_ISSUES,
    ReactionAggregateComparison, ReactionReconciliationIssue, ReactionReconciliationReceipt,
    ReactionReconciliationReport, ReactionReconciliationRequest, ReactionReconciliationStatus,
    RepairReactionSubjectCommand,
};
#[cfg(feature = "server")]
pub use service::ReactionsService;

#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub use module::ReactionsModule;

#[cfg(all(test, feature = "server"))]
mod contract_tests;
