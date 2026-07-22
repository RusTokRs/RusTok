//! Trusted process boundary for native static-distribution CI execution.

mod executor;
mod job;
mod publisher;

pub use executor::{
    StaticDistributionJobReceipt, StaticDistributionJobRequest, StaticDistributionWorker,
};
pub use job::{
    PreparedStaticDistributionWorkspace, StaticDistributionJobConfig, StaticDistributionJobError,
    StaticDistributionJobPaths, StaticDistributionPublicationReceipt,
    StaticDistributionPublisherRequest, StaticDistributionTestEvidence,
    materialize_static_distribution_workspace, run_static_distribution_job,
};
pub use publisher::{
    StaticDistributionPublisherConfig, StaticDistributionPublisherError,
    StaticDistributionPublisherPaths, run_static_distribution_publisher,
};
