pub mod dto;
pub mod health;
pub mod rollout;
#[cfg(feature = "server")]
pub mod service;
pub mod transport;

#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_core::permissions::{Action, Permission, Resource};
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
        assert_eq!(
            error_kinds,
            vec!["validation", "sanitize", "runtime", "feature-disabled"]
        );
        assert_eq!(PAGE_BUILDER_ERROR_CATALOG[3].key, "feature_disabled");
        assert_eq!(
            PAGE_BUILDER_ERROR_CATALOG[3].code,
            Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE)
        );
    }

    #[test]
    fn rollout_flags_enforce_publish_depends_on_preview() {
        let flags = BuilderCapabilityFlags {
            builder_enabled: true,
            preview_enabled: false,
            properties_enabled: true,
            publish_enabled: true,
            legacy_bridge_readonly: true,
        };

        let err = flags.validate().expect_err("invalid combination expected");
        assert_eq!(
            err,
            BuilderRolloutError::InvalidFlagCombination(
                "publish_enabled requires preview_enabled".to_string()
            )
        );
    }

    #[test]
    fn rollout_flags_enforce_builder_master_toggle() {
        let flags = BuilderCapabilityFlags {
            builder_enabled: false,
            preview_enabled: true,
            properties_enabled: false,
            publish_enabled: false,
            legacy_bridge_readonly: true,
        };

        let err = flags.validate().expect_err("invalid combination expected");
        assert_eq!(
            err,
            BuilderRolloutError::InvalidFlagCombination(
                "builder_enabled=false requires preview/properties/publish=false".to_string()
            )
        );
    }

    #[test]
    fn ensure_capability_returns_typed_disabled_error() {
        let flags = BuilderCapabilityFlags {
            builder_enabled: true,
            preview_enabled: true,
            properties_enabled: true,
            publish_enabled: false,
            legacy_bridge_readonly: false,
        };

        let err = ensure_capability(&flags, BuilderCapabilityKind::Publish)
            .expect_err("publish should be disabled");
        assert_eq!(err, BuilderRolloutError::CapabilityDisabled("publish"));
    }

    #[test]
    fn rollout_toggle_profiles_match_baseline_matrix() {
        let profiles = BuilderToggleProfile::ALL;
        let names: Vec<_> = profiles.iter().map(|profile| profile.as_str()).collect();
        assert_eq!(
            names,
            vec!["all_on", "publish_off", "preview_off", "builder_off"]
        );

        for profile in profiles {
            profile
                .flags()
                .validate()
                .unwrap_or_else(|err| panic!("profile {} must be valid: {err}", profile.as_str()));
        }

        let publish_off = BuilderToggleProfile::PublishOff.flags();
        assert!(publish_off.is_allowed(BuilderCapabilityKind::Preview));
        assert!(publish_off.is_allowed(BuilderCapabilityKind::Properties));
        assert!(!publish_off.is_allowed(BuilderCapabilityKind::Publish));

        let preview_off = BuilderToggleProfile::PreviewOff.flags();
        assert!(!preview_off.is_allowed(BuilderCapabilityKind::Preview));
        assert!(preview_off.is_allowed(BuilderCapabilityKind::Properties));
        assert!(!preview_off.is_allowed(BuilderCapabilityKind::Publish));

        let builder_off = BuilderToggleProfile::BuilderOff.flags();
        assert!(!builder_off.is_allowed(BuilderCapabilityKind::Preview));
        assert!(!builder_off.is_allowed(BuilderCapabilityKind::Properties));
        assert!(!builder_off.is_allowed(BuilderCapabilityKind::Publish));
    }

    #[test]
    fn provider_health_contract_matches_registry_baseline() {
        let states: Vec<_> = ProviderHealthState::ALL
            .iter()
            .map(|state| state.as_str())
            .collect();
        assert_eq!(states, vec!["ready", "degraded", "unavailable"]);

        let reasons: Vec<_> = ProviderDegradationReason::ALL
            .iter()
            .map(|reason| reason.as_str())
            .collect();
        assert_eq!(
            reasons,
            vec![
                "capability_disabled",
                "provider_unhealthy",
                "sanitize_backpressure",
                "publish_backlog",
            ]
        );

        assert_eq!(ProviderSloThresholds::PILOT.preview_p95_ms, 1500);
        assert_eq!(ProviderSloThresholds::PILOT.publish_p95_ms, 3000);
        assert_eq!(ProviderSloThresholds::PILOT.sanitize_failure_rate_max, 0.01);
        assert_eq!(ProviderSloThresholds::PILOT.runtime_error_rate_max, 0.01);
    }

    #[test]
    fn provider_health_snapshot_evaluates_slo_degradation() {
        let ready = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1200,
            publish_p95_ms: 2500,
            sanitize_failure_rate: 0.001,
            runtime_error_rate: 0.001,
        });
        assert_eq!(ready.state, ProviderHealthState::Ready);
        assert!(ready.degradation_reasons.is_empty());

        let degraded = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1200,
            publish_p95_ms: 3500,
            sanitize_failure_rate: 0.02,
            runtime_error_rate: 0.001,
        });
        assert_eq!(degraded.state, ProviderHealthState::Degraded);
        assert_eq!(
            degraded.degradation_reasons,
            vec![
                ProviderDegradationReason::SanitizeBackpressure,
                ProviderDegradationReason::PublishBacklog,
            ]
        );

        let unavailable = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1200,
            publish_p95_ms: 2500,
            sanitize_failure_rate: 0.001,
            runtime_error_rate: 0.03,
        });
        assert_eq!(unavailable.state, ProviderHealthState::Unavailable);
        assert_eq!(
            unavailable.degradation_reasons,
            vec![ProviderDegradationReason::ProviderUnhealthy]
        );
    }

    #[test]
    fn provider_health_evidence_exposes_wave_slo_evaluation() {
        let evidence = ProviderHealthEvidence::from_observations(ProviderSloObservations {
            preview_p95_ms: 1200,
            publish_p95_ms: 3500,
            sanitize_failure_rate: 0.001,
            runtime_error_rate: 0.001,
        });

        assert_eq!(evidence.module_slug, "page_builder");
        assert_eq!(evidence.contract, "grapesjs_v1");
        assert_eq!(evidence.builder_contract_version, "1.0");
        assert_eq!(evidence.snapshot.state, ProviderHealthState::Degraded);
        assert_eq!(
            evidence.slo_evaluation.preview_p95_ms,
            ProviderSloStatus::Pass
        );
        assert_eq!(
            evidence.slo_evaluation.publish_p95_ms,
            ProviderSloStatus::Fail
        );
        assert_eq!(evidence.slo_evaluation.overall, ProviderSloStatus::Fail);
    }

    #[test]
    fn fallback_matrix_declares_stable_runtime_outcomes() {
        let matrix = fallback_matrix();
        assert_eq!(matrix.len(), 4);

        let publish_off = BuilderToggleProfile::PublishOff.fallback_outcome();
        assert_eq!(publish_off.publish, "typed_feature_disabled_error");
        assert_eq!(publish_off.read_paths, "stable");
        assert_eq!(publish_off.disabled_capabilities, &["publish"]);

        let builder_off = BuilderToggleProfile::BuilderOff.fallback_outcome();
        assert_eq!(builder_off.admin_visual_path, "readonly_fallback");
        assert_eq!(builder_off.preview, "typed_feature_disabled_error");
        assert_eq!(builder_off.properties, "typed_feature_disabled_error");
        assert_eq!(builder_off.publish, "typed_feature_disabled_error");
        assert_eq!(
            builder_off.disabled_capabilities,
            &["preview", "tree", "properties", "publish"]
        );
    }

    #[test]
    fn module_manifest_declares_provider_contract_version() {
        let manifest = include_str!("../rustok-module.toml");
        let value: toml::Value =
            toml::from_str(manifest).expect("rustok-module.toml must stay valid TOML");

        let provider = value
            .get("fba")
            .and_then(|fba| fba.get("provider"))
            .expect("fba.provider metadata is required");

        assert_eq!(
            provider
                .get("contract")
                .and_then(toml::Value::as_str)
                .expect("fba.provider.contract is required"),
            "grapesjs_v1",
            "provider contract drifted"
        );
        assert_eq!(
            provider
                .get("builder_contract_version")
                .and_then(toml::Value::as_str)
                .expect("fba.provider.builder_contract_version is required"),
            "1.0",
            "provider builder contract version drifted"
        );
        assert_eq!(
            provider
                .get("consumer_min_version")
                .and_then(toml::Value::as_str)
                .expect("fba.provider.consumer_min_version is required"),
            "1.0",
            "provider consumer minimum version drifted"
        );
    }
}
