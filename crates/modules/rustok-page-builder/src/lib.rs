pub mod adapters;
pub mod browser_host;
#[cfg(feature = "server")]
pub mod composition;
pub mod document;
pub mod domain;
pub mod dto;
pub mod health;
pub mod interaction_providers;
pub mod landing;
#[cfg(feature = "server")]
pub mod landing_service;
pub mod locale;
#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub mod preview_port;
pub mod publish_runtime;
pub mod publish_sanitization;
pub mod render;
pub mod rollout;
pub mod runtime_context;
pub mod runtime_context_compatibility;
pub mod runtime_context_dependency;
pub mod runtime_context_migration;
pub mod runtime_scenario_release;
pub mod runtime_scenario_render;
pub mod runtime_scenario_snapshot;
#[cfg(feature = "server")]
pub mod runtime_telemetry;
#[cfg(feature = "server")]
pub mod service;
pub mod static_landing;
pub mod static_landing_materialization;
pub mod static_publish_policy;
pub mod transport;

pub use document::{PAGE_BUILDER_DOCUMENT_FORMAT, validate_page_builder_document};
pub use domain::{
    ComponentRegistryContract, ComponentRegistryManifest, CurrentContextGateMode,
    FLY_HTML_RENDERER_ID, FLY_LANDING_PROPERTY_FIELD, FLY_LANDING_SECTION_FIELD,
    FLY_RUNTIME_SCENARIO_RELEASE_BASELINE, FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT,
    FlyHtmlLandingRenderer, LandingPropertyEditError, LandingPropertyIssue,
    LandingPropertyIssueKind, LandingPropertySchema, LandingPropertySnapshot,
    LandingPropertyTargetSnapshot, LandingPropertyValidationReport, LandingReadinessCategory,
    LandingReadinessCategorySummary, LandingReadinessIssue, LandingReadinessPolicy,
    LandingReadinessReport, LandingRenderedPage, LandingRenderer, LandingRendererManifest,
    LandingSectionIssue, LandingSectionIssueKind, LandingSectionKind, LandingSectionPageManifest,
    LandingSectionPropertySnapshot, LandingSectionSnapshot, LandingSectionValidationReport,
    RegistryCompatibilityIssue, RegistryCompatibilityIssueKind, RegistryCompatibilityReport,
    RuntimeContextScenario, RuntimeContextScenarioResult, RuntimeContextScenarioSuiteResult,
    RuntimePublishGateEvaluation, RuntimePublishGatePolicy, RuntimeScenarioRegressionStatus,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleaseMode,
    RuntimeScenarioReleasePolicy, RuntimeScenarioReleaseStatus, RuntimeScenarioRenderCase,
    RuntimeScenarioRenderChange, RuntimeScenarioRenderChangeImpact, RuntimeScenarioRenderDiff,
    RuntimeScenarioRenderMatrix, RuntimeScenarioRenderSnapshot, RuntimeScenarioRenderSnapshotCase,
    ScenarioGateMode, StaticLandingArtifact, StaticLandingBuildIdentity, StaticLandingBuildResult,
    StaticLandingPage, build_static_landing_artifact, build_static_landing_artifact_with_renderer,
    diff_runtime_scenario_render_snapshots, evaluate_landing_readiness,
    evaluate_landing_readiness_with_context, evaluate_runtime_publish_gate,
    evaluate_runtime_publish_gate_with_capabilities, evaluate_runtime_scenario_release,
    landing_property_schemas, preflight_runtime_context_scenarios, render_runtime_scenario_matrix,
    render_runtime_scenario_matrix_with_providers, validate_runtime_context_scenarios,
};
pub use fly::{PageHead, RuntimeContextExamplePolicy};
pub use interaction_providers::{
    FORMS_PROVIDER_ID, FORMS_SUBMIT_ENDPOINT_TEMPLATE, FORMS_SUBMIT_OPERATION,
    HostFormProviderEndpoints, evaluate_host_runtime_publish_gate,
    evaluate_host_runtime_publish_gate_with_policy, host_interaction_capability_policy,
    host_interaction_capability_registry,
};
pub use publish_runtime::{
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_FORMAT, PageBuilderPublishRuntimeReviewError,
    PageBuilderReviewedPublishRuntime,
};
pub use publish_sanitization::{
    PAGE_BUILDER_STATIC_SANITIZATION_FORMAT, PageBuilderSanitizedStaticLandingProject,
    PageBuilderStaticLandingSanitizationError, sanitize_static_landing_project,
};
pub use static_landing_materialization::{
    PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT, PageBuilderMaterializedStaticLandingArtifact,
    PageBuilderStaticLandingMaterializationError, PageBuilderStaticLandingMaterializationIdentity,
    compile_materialized_static_landing,
};
pub use static_publish_policy::{
    PAGE_BUILDER_STATIC_PUBLISH_POLICY_FORMAT, PageBuilderStaticPublishPolicy,
    PageBuilderStaticPublishPolicyDiagnostic, PageBuilderStaticPublishPolicyError,
    PageBuilderStaticPublishPolicyEvidence, validate_static_publish_document,
};

#[cfg(feature = "server")]
pub use module::PageBuilderModule;

#[cfg(all(test, feature = "server"))]
mod contract_tests;

#[cfg(feature = "server")]
#[cfg(test)]
mod tests {
    use crate::dto::{
        BuilderCapabilityKind, BuilderNodePropertiesInput, PAGE_BUILDER_ERROR_CATALOG,
        PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE, PageBuilderCapabilityRequest,
        PageBuilderCapabilityResponse, PageBuilderErrorKind, PageBuilderModuleMetadata,
        PageBuilderPreviewRuntime, PreviewPageBuilderInput, PreviewPageBuilderResult,
        PublishPageBuilderInput, PublishPageBuilderResult,
    };
    use crate::health::{
        ProviderDegradationReason, ProviderHealthEvidence, ProviderHealthSnapshot,
        ProviderHealthState, ProviderSloObservations, ProviderSloStatus,
    };
    use crate::rollout::{
        BuilderRolloutError, BuilderToggleProfile, ensure_capability, fallback_matrix,
    };


    #[test]
    fn dto_contract_roundtrip_is_stable() {
        let input =
            PublishPageBuilderInput::new("home", "rev-1", serde_json::json!({ "pages": [] }));
        let encoded = serde_json::to_value(&input).expect("serialize input");
        assert_eq!(encoded["page_id"], "home");

        let preview = PreviewPageBuilderInput::new("home", serde_json::json!({ "pages": [] }))
            .with_runtime(PageBuilderPreviewRuntime::new(
                serde_json::json!({ "page": { "title": "Welcome" } }),
                Some("desktop".to_string()),
            ));
        let preview_json = serde_json::to_value(&preview).expect("serialize preview input");
        assert_eq!(
            preview_json["runtime"]["context"]["page"]["title"],
            "Welcome"
        );
        assert_eq!(preview_json["runtime"]["scenario_id"], "desktop");
        let decoded_preview: PreviewPageBuilderInput =
            serde_json::from_value(preview_json).expect("deserialize preview input");
        assert_eq!(decoded_preview, preview);

        let legacy_preview: PreviewPageBuilderInput = serde_json::from_value(serde_json::json!({
            "page_id": "home",
            "project_data": { "pages": [] }
        }))
        .expect("legacy preview payload uses default runtime");
        assert!(legacy_preview.runtime.context.is_object());
        assert_eq!(legacy_preview.runtime.scenario_id, None);

        let preview_result = PreviewPageBuilderResult {
            page_id: "home".to_string(),
            html: "<main/>".to_string(),
            runtime_scenario_id: Some("desktop".to_string()),
        };
        let preview_result_json =
            serde_json::to_value(&preview_result).expect("serialize preview result");
        assert_eq!(preview_result_json["runtime_scenario_id"], "desktop");

        let props = BuilderNodePropertiesInput {
            page_id: "home".to_string(),
            node_id: "hero".to_string(),
            properties: serde_json::json!({ "title": "Welcome" }),
        };
        let props_json = serde_json::to_value(&props).expect("serialize props");
        assert_eq!(props_json["node_id"], "hero");

        let result = PublishPageBuilderResult {
            page_id: "home".to_string(),
            revision_id: "rev-2".to_string(),
            published: true,
        };
        assert!(result.published);

        let request = PageBuilderCapabilityRequest::Publish(input);
        assert_eq!(request.capability(), BuilderCapabilityKind::Publish);
        let response = PageBuilderCapabilityResponse::Publish(result);
        assert_eq!(response.capability(), BuilderCapabilityKind::Publish);
        assert_eq!(
            BuilderCapabilityKind::Publish.as_str(),
            "publish",
            "capability enum string contract must stay stable"
        );

        let metadata = PageBuilderModuleMetadata::CURRENT;
        assert_eq!(metadata.module_slug, "page_builder");
        assert_eq!(
            metadata.capabilities,
            &["preview", "tree", "properties", "publish"]
        );

        let error_kinds: Vec<_> = PageBuilderErrorKind::ALL
            .iter()
            .map(|kind| kind.as_str())
            .collect();
        let catalog_kinds: Vec<_> = PAGE_BUILDER_ERROR_CATALOG
            .iter()
            .map(|entry| entry.kind.as_str())
            .collect();
        assert_eq!(error_kinds, catalog_kinds);
        assert_eq!(
            PAGE_BUILDER_ERROR_CATALOG[3].code,
            Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE)
        );
        assert_eq!(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE, "FEATURE_DISABLED");
    }

    #[test]
    fn fallback_matrix_keeps_read_paths_alive() {
        let matrix = fallback_matrix();
        assert_eq!(matrix.len(), 4);
        assert!(matrix.iter().all(|row| row.read_paths == "stable"));
        assert!(
            matrix[0].disabled_capabilities.is_empty(),
            "the fully enabled profile must expose every builder capability"
        );
    }

    #[test]
    fn disabled_capability_returns_typed_error() {
        let flags = BuilderToggleProfile::PublishOff.flags();
        let error = ensure_capability(&flags, BuilderCapabilityKind::Publish)
            .expect_err("publish should be disabled");
        assert_eq!(
            error,
            BuilderRolloutError::CapabilityDisabled(BuilderCapabilityKind::Publish.as_str())
        );
    }

    #[test]
    fn health_snapshot_roundtrip_is_stable() {
        let observed = ProviderSloObservations {
            preview_p95_ms: 820,
            publish_p95_ms: 1_200,
            sanitize_failure_rate: 0.02,
            runtime_error_rate: 0.005,
        };
        let snapshot = ProviderHealthSnapshot::evaluate(observed);
        assert_eq!(snapshot.state, ProviderHealthState::Degraded);
        assert_eq!(
            snapshot.degradation_reasons,
            vec![ProviderDegradationReason::SanitizeBackpressure]
        );
        let value = serde_json::to_value(&snapshot).expect("serialize health snapshot");
        let decoded: ProviderHealthSnapshot =
            serde_json::from_value(value).expect("deserialize health snapshot");
        assert_eq!(decoded, snapshot);

        let evidence = ProviderHealthEvidence::from_observations(observed);
        assert_eq!(evidence.module_slug, "page_builder");
        assert_eq!(evidence.builder_contract_version, "1.1");
        assert_eq!(evidence.slo_evaluation.overall, ProviderSloStatus::Fail);
    }
}
