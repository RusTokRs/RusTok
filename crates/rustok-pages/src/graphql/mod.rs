mod mutation;
mod query;
mod scenario_baseline;
mod types;

use async_graphql::MergedObject;

#[derive(MergedObject, Default)]
pub struct PagesQuery(query::PagesQuery, scenario_baseline::PageBuilderScenarioBaselineQuery);

#[derive(MergedObject, Default)]
pub struct PagesMutation(
    mutation::PagesMutation,
    scenario_baseline::PageBuilderScenarioBaselineMutation,
);

pub use scenario_baseline::{
    GqlPageBuilderScenarioBaseline, SaveGqlPageBuilderScenarioBaselineInput,
};
pub use types::*;
