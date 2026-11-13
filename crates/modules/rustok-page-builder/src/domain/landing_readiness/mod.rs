mod evaluate;
mod model;

pub use evaluate::{evaluate_landing_readiness, evaluate_landing_readiness_with_context};
pub(crate) use evaluate::{
    evaluate_materialized_landing_readiness, materialize_landing_structural_document,
};
pub use model::{
    LandingReadinessCategory, LandingReadinessCategorySummary, LandingReadinessIssue,
    LandingReadinessPolicy, LandingReadinessReport,
};

#[cfg(test)]
mod tests;
