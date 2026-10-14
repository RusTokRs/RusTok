//! Landing-page and runtime-scenario domain owned by the Page Builder module.
//!
//! Extracted from the `fly` core, which stays a framework-neutral engine
//! (DECISIONS/2026-10-14-landing-scenario-domain-leaves-fly-core.md). The facade
//! re-exports the domain surface explicitly; `rustok-page-builder` in turn
//! re-exports these names at its crate root for hosts.

mod context_scenario;
mod interaction_capability_gate;
mod landing_contract;
mod landing_property;
mod landing_readiness;
mod runtime_gate;
mod runtime_scenario_release;
mod runtime_scenario_render;
mod runtime_scenario_snapshot;

pub use context_scenario::{
    RuntimeContextScenario, RuntimeContextScenarioResult, RuntimeContextScenarioSuiteResult,
    preflight_runtime_context_scenarios, validate_runtime_context_scenarios,
};

pub use interaction_capability_gate::evaluate_runtime_publish_gate_with_capabilities;

pub use landing_contract::{
    ComponentRegistryContract, ComponentRegistryManifest, FLY_HTML_RENDERER_ID,
    FLY_LANDING_SECTION_FIELD, FlyHtmlLandingRenderer, LandingRenderedPage, LandingRenderer,
    LandingRendererManifest, LandingSectionIssue, LandingSectionIssueKind, LandingSectionKind,
    LandingSectionPageManifest, LandingSectionSnapshot, LandingSectionValidationReport,
    RegistryCompatibilityIssue, RegistryCompatibilityIssueKind, RegistryCompatibilityReport,
    StaticLandingArtifact, StaticLandingBuildIdentity, StaticLandingBuildResult, StaticLandingPage,
    build_static_landing_artifact, build_static_landing_artifact_with_renderer,
};

pub use landing_property::{
    FLY_LANDING_PROPERTY_FIELD, LandingPropertyEditError, LandingPropertyIssue,
    LandingPropertyIssueKind, LandingPropertySchema, LandingPropertySnapshot,
    LandingPropertyTargetSnapshot, LandingPropertyValidationReport, LandingSectionPropertySnapshot,
    landing_property_schemas,
};

pub use landing_readiness::{
    LandingReadinessCategory, LandingReadinessCategorySummary, LandingReadinessIssue,
    LandingReadinessPolicy, LandingReadinessReport, evaluate_landing_readiness,
    evaluate_landing_readiness_with_context,
};

pub use runtime_gate::{
    CurrentContextGateMode, RuntimePublishGateEvaluation, RuntimePublishGatePolicy,
    ScenarioGateMode, evaluate_runtime_publish_gate,
};

pub use runtime_scenario_release::{
    FLY_RUNTIME_SCENARIO_RELEASE_BASELINE, RuntimeScenarioReleaseBaseline,
    RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleaseMode, RuntimeScenarioReleasePolicy,
    RuntimeScenarioReleaseStatus, evaluate_runtime_scenario_release,
};

pub use runtime_scenario_render::{
    RuntimeScenarioRenderCase, RuntimeScenarioRenderMatrix, render_runtime_scenario_matrix,
    render_runtime_scenario_matrix_with_providers,
};

pub use runtime_scenario_snapshot::{
    FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT, RuntimeScenarioRegressionStatus,
    RuntimeScenarioRenderChange, RuntimeScenarioRenderChangeImpact, RuntimeScenarioRenderDiff,
    RuntimeScenarioRenderSnapshot, RuntimeScenarioRenderSnapshotCase,
    diff_runtime_scenario_render_snapshots,
};
