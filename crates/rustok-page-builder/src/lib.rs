pub mod adapters;
pub mod dto;
mod dto_display;
pub mod health;
pub mod rollout;
#[cfg(feature = "server")]
pub mod service;
pub mod transport;

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
        BuilderCapabilityKind, BuilderNodePropertiesInput, PageBuilderCapabilityRequest,
        PageBuilderCapabilityResponse, PageBuilderContractMetadata, PageBuilderErrorKind,
        PublishPageBuilderInput, PublishPageBuilderResult, PAGE_BUILDER_ERROR_CATALOG,
        PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE,
    };
    use crate::health::{
        ProviderDegradationReason, ProviderHealthEvidence, ProviderHealthSnapshot,
        ProviderHealthState, ProviderSloObservations, ProviderSloStatus, ProviderSloThresholds,
    };
    use crate::rollout::{
        ensure_capability, fallback_matrix, BuilderCapabilityFlags, BuilderRolloutError,
        BuilderToggleProfile,
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
        let input = PublishPageBuilderInput {
            page_id: "home".to_string(),
            revision_id: "rev-1".to_string(),
            schema_version: "grapesjs_v1".to_string(),
            project_data: serde_json::json!({ "pages": [] }),
        };
        let encoded = serde_json::to_string(&input).expect("serialize input");
        let decoded: PublishPageBuilderInput =
            serde_json::from_str(&encoded).expect("deserialize input");
        assert_eq!(decoded.page_id, "home");
        assert_eq!(decoded.schema_version, "grapesjs_v1");

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

        let metadata = PageBuilderContractMetadata::BASELINE;
        assert_eq!(metadata.module_slug, "page_builder");
        assert_eq!(metadata.contract, "grapesjs_v1");
        assert_eq!(metadata.builder_contract_version, "1.0");
        assert_eq!(metadata.consumer_min_version, "1.0");
        assert_eq!(
            metadata.capabilities,
            &["preview", "tree", "properties", "publish"]
        );

        let error_kinds: Vec<_> = PageBuilderErrorKind::ALL
            .iter()
            .map(|kind| kind.as_str())
            .collect();
        assert_eq!(error_kinds, PAGE_BUILDER_ERROR_CATALOG);
        assert_eq!(
            PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE,
            "FEATURE_DISABLED"
        );
    }

    #[test]
    fn fallback_matrix_keeps_read_paths_alive() {
        let matrix = fallback_matrix();
        assert_eq!(matrix.len(), 4);
        assert!(matrix
            .iter()
            .all(|row| row.profile != BuilderToggleProfile::AllOn || row.tree_available));
    }

    #[test]
    fn disabled_capability_returns_typed_error() {
        let flags = BuilderCapabilityFlags::from_profile(BuilderToggleProfile::PublishOff);
        let error = ensure_capability(&flags, BuilderCapabilityKind::Publish)
            .expect_err("publish should be disabled");
        assert!(matches!(error, BuilderRolloutError::FeatureDisabled(_)));
        assert_eq!(error.stable_code(), PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE);
    }

    #[test]
    fn health_snapshot_roundtrip_is_stable() {
        let snapshot = ProviderHealthSnapshot {
            state: ProviderHealthState::Degraded,
            reason: Some(ProviderDegradationReason::SanitizeBackpressure),
            evidence: ProviderHealthEvidence {
                observed_at: "2026-07-13T00:00:00Z".to_string(),
                slo_status: ProviderSloStatus::Violated,
                thresholds: ProviderSloThresholds {
                    preview_p95_ms: 750,
                    publish_p95_ms: 1_500,
                    error_rate_bps: 100,
                },
                observations: ProviderSloObservations {
                    preview_p95_ms: 820,
                    publish_p95_ms: 1_200,
                    error_rate_bps: 50,
                },
            },
        };
        let value = serde_json::to_value(&snapshot).expect("serialize health snapshot");
        let decoded: ProviderHealthSnapshot =
            serde_json::from_value(value).expect("deserialize health snapshot");
        assert_eq!(decoded, snapshot);
    }
}
