mod artifact_integrity_audit;
mod mutation;
mod query;
mod scenario_baseline;
mod types;

use async_graphql::MergedObject;

#[derive(MergedObject, Default)]
pub struct PagesQuery(
    query::PagesQuery,
    scenario_baseline::PageBuilderScenarioBaselineQuery,
);

#[derive(MergedObject, Default)]
pub struct PagesMutation(
    mutation::PagesMutation,
    scenario_baseline::PageBuilderScenarioBaselineMutation,
    artifact_integrity_audit::PageArtifactIntegrityAuditMutation,
);

pub use artifact_integrity_audit::{
    AuditGqlPageArtifactsInput, GqlPageArtifactIntegrityAuditResult,
    GqlPageArtifactIntegrityFinding,
};
pub use scenario_baseline::{
    GqlPageBuilderScenarioBaseline, GqlPageBuilderScenarioReleaseStatus,
    SaveGqlPageBuilderScenarioBaselineInput,
};
pub use types::*;
