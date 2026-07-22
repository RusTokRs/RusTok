pub mod adapters;
pub mod browser_host;
pub mod dto;
pub mod health;
pub mod landing;
#[cfg(feature = "server")]
pub mod landing_service;
pub mod locale;
pub mod render;
pub mod rollout;
pub mod runtime_context;
pub mod runtime_context_dependency;
pub mod runtime_scenario_release;
pub mod runtime_scenario_render;
pub mod runtime_scenario_snapshot;
#[cfg(feature = "server")]
pub mod runtime_telemetry;
#[cfg(feature = "server")]
pub mod service;
pub mod static_landing;
pub mod transport;

pub use fly::{
    ComponentRegistryManifest, LandingRenderer, LandingRendererManifest, LandingSectionSnapshot,
    PageHead, RuntimeContextExamplePolicy, RuntimeContextScenario, StaticLandingArtifact,
    StaticLandingBuildIdentity, StaticLandingPage,
};

#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_api::{Action, Permission, Resource};
#[cfg(feature = "server")]
use rustok_core::{MigrationSource, RusToKModule};
#[cfg(feature = "server")]
use sea_orm_migration::MigrationTrait;

#[cfg(feature = "server")]
pub struct PageBuilderModule;

#[cfg(feature = "server")]
#[async_trait]
impl RusToKModule for PageBuilderModule {
    fn slug(&self) -> &'static str {
        "page_builder"
    }

    fn name(&self) -> &'static str {
        "Page Builder"
    }

    fn description(&self) -> &'static str {
        "Standalone FBA-first visual page builder reference module"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::Publish),
            Permission::new(Resource::Pages, Action::Manage),
        ]
    }
}

#[cfg(feature = "server")]
impl MigrationSource for PageBuilderModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(feature = "server")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{
        BuilderCapabilityKind, BuilderNodePropertiesInput, PAGE_BUILDER_ERROR_CATALOG,
        PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE, PageBuilderCapabilityRequest,
        PageBuilderCapabilityResponse, PageBuilderErrorKind, PageBuilderModuleMetadata,
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
    fn module_metadata_is_stable() {
        let module = PageBuilderModule;

        assert_eq!(module.slug(), "page_builder");
        assert_eq!(module.name(), "Page Builder");
        assert_eq!(
            module.description(),
            "Standalone FBA-first visual page builder reference module"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn dto_contract_roundtrip_is_stable() {
        let input =
            PublishPageBuilderInput::new("home", "rev-1", serde_json::json!({ "pages": [] }));
        let encoded = serde_json::to_value(&input).expect("serialize input");
        assert_eq!(encoded["page_id"], "home");

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
        assert_eq!(evidence.slo_evaluation.overall, ProviderSloStatus::Fail);
    }
}
