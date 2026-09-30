//! Shared test fixtures and builder helpers for governance test suites.

use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use super::*;
use crate::build::{
    ModuleBuildAuthoring, ModuleBuildComponentInterface, ModuleBuildDependencyPolicy,
    ModuleBuildEvidence, ModuleBuildLimits, ModuleBuildMetrics, ModuleBuildNetworkPolicy,
    ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildPublicationReceipt,
    ModuleBuildRequest, ModuleBuildResult, ModuleBuildScenario, ModuleBuildSignatureAuthority,
    ModuleBuildSource, ModuleBuildToolchain, ModuleBuildValidationOutcome,
    ModuleBuildValidationProfile, ModuleBuildValidationResult, ModuleBuildWitContract,
};
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::{
    ArtifactBlobStore, ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef,
    ControlPlaneInfrastructure, InMemoryArtifactBlobStore, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleMarketplaceArtifactOrigin, ModuleMarketplaceArtifactRelease,
    ModuleMarketplaceEvidenceKind, ModuleMarketplaceEvidenceReference, TrustEvidenceKind,
    TrustEvidenceReference, MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_PROTOCOL_VERSION,
    MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION, MODULE_BUILD_WIT_WORLD,
};

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

    pub(crate) fn completed_platform_build(
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
            toolchain: ModuleBuildToolchain {
                rust_toolchain: "1.85.0".to_string(),
                component_target: MODULE_BUILD_COMPONENT_TARGET.to_string(),
            },
            authoring: ModuleBuildAuthoring {
                sdk_version: "1.0.0".to_string(),
                template_version: "1.0.0".to_string(),
            },
            dependency_policy: ModuleBuildDependencyPolicy {
                lock_digest: stage_digest('b'),
                allowed_registries: vec!["https://crates.io".to_string()],
                allow_git_dependencies: false,
                allow_build_scripts: false,
                allow_native_links: false,
            },
            limits: ModuleBuildLimits {
                cpu_cores: 2,
                memory_bytes: 512 * 1024 * 1024,
                disk_bytes: 2 * 1024 * 1024 * 1024,
                process_limit: 32,
                output_bytes: 1024 * 1024,
                wall_clock_ms: 300_000,
            },
            network_policy: ModuleBuildNetworkPolicy::Denied,
            validation_profiles: vec![
                ModuleBuildValidationProfile::Check,
                ModuleBuildValidationProfile::Test,
                ModuleBuildValidationProfile::DependencyPolicy,
                ModuleBuildValidationProfile::Vulnerability,
            ],
            attempt: 1,
        };
        let reference = |marker| OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
            digest: stage_digest(marker),
        };
        let result = ModuleBuildResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            tenant_id,
            attempt: request.attempt,
            outcome: ModuleBuildOutcome::Succeeded,
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_digest: Some(stage_digest('c')),
            sbom_digest: Some(stage_digest('d')),
            provenance_digest: Some(stage_digest('e')),
            component_interface: Some(ModuleBuildComponentInterface {
                exports: vec!["run".to_string()],
                imports: Vec::new(),
            }),
            evidence: ModuleBuildEvidence {
                log_reference: "cas://logs/platform-build".to_string(),
                policy_report_reference: "cas://reports/platform-build".to_string(),
                validation_results: request
                    .validation_profiles
                    .iter()
                    .copied()
                    .map(|profile| ModuleBuildValidationResult {
                        profile,
                        outcome: ModuleBuildValidationOutcome::Passed,
                    })
                    .collect(),
                scenario_comparison: Some(rustok_sandbox::LocalSandboxScenarioComparison {
                    scenario_digest: request.scenario.digest.clone(),
                    result: rustok_sandbox::LocalSandboxScenarioResult::Success,
                }),
                diagnostics: Vec::new(),
            },
            publication: Some(ModuleBuildPublicationReceipt {
                artifact: reference('f'),
                signature_manifest: reference('a'),
                signature_authority: ModuleBuildSignatureAuthority::BuildService,
            }),
            metrics: ModuleBuildMetrics {
                duration_ms: 1,
                peak_memory_bytes: 1,
                output_bytes: 1,
            },
            retryable: false,
            next_action: ModuleBuildNextAction::AdmitArtifact,
        };
        (request, result)
    }


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

