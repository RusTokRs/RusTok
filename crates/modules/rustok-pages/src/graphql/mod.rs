mod artifact_integrity_audit;
mod artifact_repair;
mod builder_rollout;
mod mutation;
mod query;
mod runtime_data;
mod scenario_baseline;
mod types;

use async_graphql::MergedObject;

#[derive(MergedObject, Default)]
pub struct PagesQuery(
    query::PagesQuery,
    scenario_baseline::PageBuilderScenarioBaselineQuery,
    builder_rollout::PageBuilderRolloutQuery,
);

#[derive(MergedObject, Default)]
pub struct PagesMutation(
    mutation::PagesMutation,
    scenario_baseline::PageBuilderScenarioBaselineMutation,
    artifact_integrity_audit::PageArtifactIntegrityAuditMutation,
    artifact_repair::PageArtifactRepairMutation,
);

pub use artifact_integrity_audit::{
    AuditGqlPageArtifactsInput, GqlPageArtifactIntegrityAuditResult,
    GqlPageArtifactIntegrityFinding,
};
pub use artifact_repair::{
    ActivateGqlRebuiltPageArtifactInput, GqlActivateRebuiltPageArtifactResult,
    GqlRebuildPageArtifactResult, RebuildGqlPageArtifactInput,
};
pub use builder_rollout::{
    GqlPageBuilderCapability, GqlPageBuilderCapabilityPreflight, GqlPageBuilderRolloutSnapshot,
};
pub use runtime_data::PagesGraphqlRuntimeData;
pub use runtime_data::attach_schema_data;
pub use scenario_baseline::{
    GqlPageBuilderScenarioBaseline, GqlPageBuilderScenarioReleaseStatus,
    SaveGqlPageBuilderScenarioBaselineInput,
};
pub use types::*;
