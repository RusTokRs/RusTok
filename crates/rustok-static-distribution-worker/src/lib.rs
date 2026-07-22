//! Trusted process boundary for native static-distribution CI execution.

mod executor;
mod job;
mod publisher;

pub use executor::{
    StaticDistributionJobReceipt, StaticDistributionJobRequest, StaticDistributionWorker,
};
pub use job::{
    materialize_static_distribution_workspace, run_static_distribution_job,
    PreparedStaticDistributionWorkspace, StaticDistributionJobConfig, StaticDistributionJobError,
    StaticDistributionJobPaths, StaticDistributionPublicationReceipt,
    StaticDistributionPublisherRequest, StaticDistributionTestEvidence,
};
pub use publisher::{
    run_static_distribution_publisher, StaticDistributionPublisherConfig,
    StaticDistributionPublisherError, StaticDistributionPublisherPaths,
};
