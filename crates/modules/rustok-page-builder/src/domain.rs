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
    preflight_runtime_context_scenarios, validate_runtime_context_scenarios,
    RuntimeContextScenario, RuntimeContextScenarioResult, RuntimeContextScenarioSuiteResult,
};

pub use interaction_capability_gate::evaluate_runtime_publish_gate_with_capabilities;

pub use landing_contract::{
    build_static_landing_artifact, build_static_landing_artifact_with_renderer,
    ComponentRegistryContract, ComponentRegistryManifest, FlyHtmlLandingRenderer,
    LandingRenderedPage, LandingRenderer, LandingRendererManifest, LandingSectionIssue,
    LandingSectionIssueKind, LandingSectionKind, LandingSectionPageManifest,
    LandingSectionSnapshot, LandingSectionValidationReport, RegistryCompatibilityIssue,
    RegistryCompatibilityIssueKind, RegistryCompatibilityReport, StaticLandingArtifact,
    StaticLandingBuildIdentity, StaticLandingBuildResult, StaticLandingPage, FLY_HTML_RENDERER_ID,
    FLY_LANDING_SECTION_FIELD,
};

pub use landing_property::{
    landing_property_schemas, LandingPropertyEditError, LandingPropertyIssue,
    LandingPropertyIssueKind, LandingPropertySchema, LandingPropertySnapshot,
    LandingPropertyTargetSnapshot, LandingPropertyValidationReport, LandingSectionPropertySnapshot,
    FLY_LANDING_PROPERTY_FIELD,
};

pub use landing_readiness::{
    evaluate_landing_readiness, evaluate_landing_readiness_with_context, LandingReadinessCategory,
    LandingReadinessCategorySummary, LandingReadinessIssue, LandingReadinessPolicy,
    LandingReadinessReport,
};

pub use runtime_gate::{
    evaluate_runtime_publish_gate, CurrentContextGateMode, RuntimePublishGateEvaluation,
    RuntimePublishGatePolicy, ScenarioGateMode,
};

pub use runtime_scenario_release::{
    evaluate_runtime_scenario_release, RuntimeScenarioReleaseBaseline,
    RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleaseMode, RuntimeScenarioReleasePolicy,
    RuntimeScenarioReleaseStatus, FLY_RUNTIME_SCENARIO_RELEASE_BASELINE,
};

pub use runtime_scenario_render::{
    render_runtime_scenario_matrix, render_runtime_scenario_matrix_with_providers,
    RuntimeScenarioRenderCase, RuntimeScenarioRenderMatrix,
};

pub use runtime_scenario_snapshot::{
    diff_runtime_scenario_render_snapshots, RuntimeScenarioRegressionStatus,
    RuntimeScenarioRenderChange, RuntimeScenarioRenderChangeImpact, RuntimeScenarioRenderDiff,
    RuntimeScenarioRenderSnapshot, RuntimeScenarioRenderSnapshotCase,
    FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT,
};
