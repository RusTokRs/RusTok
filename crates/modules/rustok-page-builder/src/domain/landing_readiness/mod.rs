mod evaluate;
mod model;

pub(crate) use evaluate::materialize_landing_structural_document;
pub use evaluate::{evaluate_landing_readiness, evaluate_landing_readiness_with_context};
pub use model::{
    LandingReadinessCategory, LandingReadinessCategorySummary, LandingReadinessIssue,
    LandingReadinessPolicy, LandingReadinessReport,
};

#[cfg(test)]
mod tests;
