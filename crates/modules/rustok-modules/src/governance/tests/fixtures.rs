//! Shared test fixtures and builder helpers for governance test suites.

use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use super::*;
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::publication_evidence::{
    ModulePublicationTrustEvidence, ModulePublicationTrustReport,
    ModulePublicationTrustSubject, ModulePublicationTrustVerifier,
};
use crate::publish_validation::{
    ModulePublishValidationCheck, ModulePublishValidationDetails,
    ModulePublishValidationResultOutcome,
};
use crate::{ControlPlaneInfrastructure, ModuleCommandContext};

    pub(crate) fn trust_evidence(digest_character: char) -> Vec<TrustEvidenceReference> {
        [
            TrustEvidenceKind::Signature,
            TrustEvidenceKind::Provenance,
            TrustEvidenceKind::Sbom,
        ]
        .into_iter()
        .map(|kind| TrustEvidenceReference {
            kind,
            reference: format!(
                "oci://registry.example/modules/sample@sha256:{}#{}",
                digest_character.to_string().repeat(64),
                kind.as_str()
            ),
            digest: format!("sha256:{}", digest_character.to_string().repeat(64)),
        })
        .collect()
    }

    pub(crate) fn alloy_descriptor(
        slug: &str,
        version: &str,
        digest_character: char,
    ) -> ModuleArtifactDescriptor {
        ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: slug.to_string(),
            version: version.to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", digest_character.to_string().repeat(64)),
            entrypoint: "src/main.rhai".to_string(),
            capabilities: Vec::new(),
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: Vec::new(),
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        }
    }

    pub(crate) fn publish_request_create_command() -> ModulePublishRequestCreateCommand {
        let actor_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4();
        ModulePublishRequestCreateCommand {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            crate_name: "sample-module".to_string(),
            default_locale: "en-US".to_string(),
            ownership: "third_party".to_string(),
            trust_level: "reviewed".to_string(),
            license: "MIT".to_string(),
            entry_type: Some("sandboxed".to_string()),
            artifact_origin: ModulePublicationArtifactOrigin::ExternalPrebuilt,
            marketplace: serde_json::json!({ "category": "utilities", "tags": [] }),
            ui_packages: serde_json::json!({ "admin": null, "storefront": null }),
            name: "Sample module".to_string(),
            description: "A publish request description long enough for policy.".to_string(),
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: format!("test:request-create:{idempotency_key}"),
                correlation_id: idempotency_key,
                idempotency_key,
            },
            actor_principal: serde_json::json!({ "kind": "user", "user_id": actor_id, "subject": format!("user:{actor_id}") }),
            actor_can_manage_modules: false,
        }
    }


    pub(crate) fn stage_digest(marker: char) -> String {
        format!("sha256:{}", marker.to_string().repeat(64))
    }

    fn completed_platform_build(
        tenant_id: Uuid,
        build_request_id: Uuid,
    ) -> (ModuleBuildRequest, ModuleBuildResult) {
        let request = ModuleBuildRequest {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: build_request_id,
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(tenant_id),
                trace_id: "test:completed-platform-build".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            project_id: "project:sample".to_string(),
            source: ModuleBuildSource {
                digest: stage_digest('a'),
                reference: format!("cas://{}", stage_digest('a')),
            },
            scenario: ModuleBuildScenario {
                source_path: "tests/sandbox-scenario.json".to_string(),
                digest: stage_digest('c'),
            },
            expected_module_slug: "sample_module".to_string(),
            expected_version: "1.0.0".to_string(),
            parent_release: None,
            runtime_abi: MODULE_BUILD_RUNTIME_ABI.to_string(),
            wit: ModuleBuildWitContract {
                world: MODULE_BUILD_WIT_WORLD.to_string(),
                version: MODULE_BUILD_WIT_VERSION.to_string(),
            },

    pub(crate) fn governance_request_snapshot(
        status: &str,
        artifact_origin: &str,
    ) -> ModuleGovernanceRequestSnapshot {
        ModuleGovernanceRequestSnapshot {
            id: "request-1".to_string(),
            revision: 1,
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            status: status.to_string(),
            artifact_origin: artifact_origin.to_string(),
            requested_by_principal: serde_json::json!({ "kind": "user", "id": "publisher" }),
            publisher_principal: Some(serde_json::json!({ "kind": "user", "id": "publisher" })),
            approved_by_principal: None,
            rejected_by_principal: None,
            rejection_reason: None,
            changes_requested_by_principal: None,
            changes_requested_reason: None,
            changes_requested_reason_code: None,
            changes_requested_at: None,
            held_by_principal: None,
            held_reason: None,
            held_reason_code: None,
            held_at: None,
            held_from_status: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            updated_at: "2026-07-22T00:00:00Z".to_string(),
            published_at: None,
        }
    }

