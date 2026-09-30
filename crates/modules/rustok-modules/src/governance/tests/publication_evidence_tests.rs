use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait, Value};
use semver::Version;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::fixtures::*;
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


    #[tokio::test]
    async fn publication_evidence_is_authority_scoped_and_idempotent() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL,\
                artifact_origin TEXT NOT NULL, artifact_checksum_sha256 TEXT NULL, status TEXT NOT NULL,\
                requested_by_principal JSON NOT NULL, publisher_principal JSON NULL,\
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
             )",
            "CREATE TABLE registry_publication_evidence (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, authority TEXT NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, evidence_reference TEXT NOT NULL,\
                issuer_identity TEXT NOT NULL, policy_revision TEXT NOT NULL,\
                signature_digest_sha256 TEXT NULL,\
                evidence_digest_sha256 TEXT NOT NULL, recorded_by_principal TEXT NOT NULL,\
                created_at TEXT NOT NULL, UNIQUE (request_id, evidence_digest_sha256)\
             )",
            "CREATE TABLE registry_author_signature_evidence_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, request_id TEXT NOT NULL,\
                idempotency_key TEXT NOT NULL, expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, evidence_reference TEXT NOT NULL,\
                signature_digest_sha256 TEXT NOT NULL, signer_identity TEXT NOT NULL,\
                policy_revision TEXT NOT NULL, evidence_id TEXT NOT NULL, resulting_revision INTEGER NOT NULL,\
                recorded INTEGER NOT NULL, committed_at TEXT NOT NULL, UNIQUE (request_id, idempotency_key)\
             )",
            "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY NOT NULL, owner_principal JSON NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, registry_id TEXT NOT NULL, registry TEXT NOT NULL,\
                repository TEXT NOT NULL, manifest_digest TEXT NOT NULL, payload_digest TEXT NOT NULL,\
                descriptor_digest TEXT NOT NULL, descriptor JSON NOT NULL, runtime_kind TEXT NOT NULL,\
                media_type TEXT NOT NULL, signature_reference TEXT NOT NULL, signature_digest TEXT NOT NULL,\
                provenance_reference TEXT NOT NULL, provenance_digest TEXT NOT NULL,\
                sbom_reference TEXT NOT NULL, sbom_digest TEXT NOT NULL,\
                admission_reference TEXT NOT NULL, admission_digest TEXT NOT NULL,\
                recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema or fixture");
        }

        let actor_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests \
                 (id, slug, version, artifact_origin, artifact_checksum_sha256, status, requested_by_principal) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                vec![
                    "request-1".into(),
                    "sample_module".into(),
                    "1.0.0".into(),
                    "platform_built".into(),
                    "a".repeat(64).into(),
                    "approved".into(),
                    serde_json::json!({
                        "kind": "user",
                        "id": actor_id,
                    })
                    .to_string()
                    .into(),
                ],
            ))
            .await
            .expect("publish request fixture");
        let command = ModuleAuthorSignatureEvidenceCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: "test:author-signature-evidence".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_can_manage_modules: false,
            evidence_reference: "oci://registry.example/modules/sample@sha256:author-signature"
                .to_string(),
            signature_digest_sha256: "b".repeat(64),
            signer_identity: "author:sample".to_string(),
            policy_revision: "author-policy-v1".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let first = service
            .record_author_signature_evidence(command.clone())
            .await
            .expect("record evidence");
        let unauthorized_actor_id = Uuid::new_v4();
        let unauthorized = ModuleAuthorSignatureEvidenceCommand {
            expected_revision: first.request_revision,
            context: ModuleCommandContext {
                actor_id: unauthorized_actor_id,
                tenant_id: None,
                trace_id: "test:author-signature-unauthorized".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_principal: serde_json::json!({ "kind": "user", "id": unauthorized_actor_id }),
            ..command.clone()
        };
        assert_eq!(
            service.record_author_signature_evidence(unauthorized).await,
            Err(ModuleGovernanceError::PublishRequestAuthorSignatureUnauthorized)
        );
        let stale_new_evidence = ModuleAuthorSignatureEvidenceCommand {
            evidence_reference:
                "oci://registry.example/modules/sample@sha256:second-author-signature".to_string(),
            ..command.clone()
        };
        assert!(matches!(
            service
                .record_author_signature_evidence(stale_new_evidence)
                .await,
            Err(ModuleGovernanceError::AuthorSignatureEvidenceIdempotencyConflict)
        ));
        let repeated = service
            .record_author_signature_evidence(command)
            .await
            .expect("repeat evidence");
        let author_signature_receipt = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT subject_digest_sha256, signature_digest_sha256, recorded \
                 FROM registry_author_signature_evidence_operations WHERE request_id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("author signature receipt query")
            .expect("author signature receipt");
        assert_eq!(
            author_signature_receipt
                .try_get::<String>("", "subject_digest_sha256")
                .expect("author signature subject"),
            "a".repeat(64)
        );
        assert_eq!(
            author_signature_receipt
                .try_get::<String>("", "signature_digest_sha256")
                .expect("signature digest"),
            "b".repeat(64)
        );
        assert!(
            author_signature_receipt
                .try_get::<bool>("", "recorded")
                .expect("initial record outcome")
        );
        let author_signature_evidence = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT subject_digest_sha256, signature_digest_sha256 \
                 FROM registry_publication_evidence WHERE authority = 'author_signature'"
                    .to_string(),
            ))
            .await
            .expect("author signature evidence query")
            .expect("author signature evidence");
        assert_eq!(
            author_signature_evidence
                .try_get::<String>("", "subject_digest_sha256")
                .expect("author signature evidence subject"),
            "a".repeat(64)
        );
        assert_eq!(
            author_signature_evidence
                .try_get::<String>("", "signature_digest_sha256")
                .expect("author signature evidence digest"),
            "b".repeat(64)
        );
        let reference = |digest: char| crate::installation::OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample".to_string(),
            digest: format!("sha256:{}", digest.to_string().repeat(64)),
        };
        let build_command = ModuleBuildServiceAttestationCommand {
            request_id: "request-1".to_string(),
            expected_revision: first.request_revision,
            receipt: ModuleBuildPublicationReceipt {
                artifact: reference('a'),
                signature_manifest: reference('d'),
                signature_authority: ModuleBuildSignatureAuthority::BuildService,
            },
            issuer_identity: "build-service:production".to_string(),
            policy_revision: "build-policy-v1".to_string(),
            actor_principal: serde_json::json!({ "kind": "service", "id": "build-worker" }),
        };
        let build = service
            .record_build_service_attestation(build_command.clone())
            .await
            .expect("record build evidence");
        let platform_admission = ModulePlatformAdmissionCommand {
            request_id: "request-1".to_string(),
            expected_revision: build.request_revision,
            registry_id: "local".to_string(),
            reference: reference('a'),
            descriptor: crate::ModuleArtifactDescriptor {
                schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                payload_kind: crate::ArtifactPayloadKind::WasmComponent,
                module_kind: crate::ArtifactModuleKind::Optional,
                runtime_abi: "rustok:module/runtime@1".to_string(),
                platform_compatibility: "^0.1".to_string(),
                required_features: Vec::new(),
                artifact_digest: format!("sha256:{}", "e".repeat(64)),
                entrypoint: "main".to_string(),
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
            },
            evidence: ArtifactVerificationEvidence {
                manifest_digest: format!("sha256:{}", "a".repeat(64)),
                payload_digest: format!("sha256:{}", "e".repeat(64)),
                media_type: ArtifactPayloadKind::WasmComponent
                    .oci_layer_media_type()
                    .to_string(),
                signer_identity: "build-service:production".to_string(),
                trust_policy_revision: 7,
                capability_policy_revision: 9,
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                license_policy_verified: true,
                vulnerability_policy_verified: true,
                evidence: trust_evidence('a'),
                verified_at: chrono::Utc::now(),
            },
            actor_principal: serde_json::json!({ "kind": "service", "id": "verification-worker" }),
        };
        assert_eq!(
            platform_admission
                .publication_evidence(ModulePublicationArtifactOrigin::ExternalPrebuilt)
                .expect("external admission evidence")
                .subject_digest_sha256,
            "e".repeat(64)
        );
        let accepted_fingerprint = platform_admission_evidence_reference(
            &platform_admission.reference,
            &platform_admission.evidence,
        );
        let mut incomplete_admission = platform_admission.clone();
        incomplete_admission.evidence.license_policy_verified = false;
        assert!(matches!(
            incomplete_admission.validate(),
            Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand)
        ));
        assert_ne!(
            accepted_fingerprint,
            platform_admission_evidence_reference(
                &incomplete_admission.reference,
                &incomplete_admission.evidence,
            )
        );
        let mut vulnerable_admission = platform_admission.clone();
        vulnerable_admission.evidence.vulnerability_policy_verified = false;
        assert!(matches!(
            vulnerable_admission.validate(),
            Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand)
        ));
        assert_ne!(
            accepted_fingerprint,
            platform_admission_evidence_reference(
                &vulnerable_admission.reference,
                &vulnerable_admission.evidence,
            )
        );
        let admission = service
            .record_platform_admission(platform_admission.clone())
            .await
            .expect("record platform admission");
        let repeated_admission = service
            .record_platform_admission(platform_admission)
            .await
            .expect("repeat platform admission");

        assert!(first.recorded);
        assert!(!repeated.recorded);
        assert_eq!(first.evidence_id, repeated.evidence_id);
        assert_eq!(first.request_revision, 2);
        assert_eq!(repeated.request_revision, 2);
        assert!(build.recorded);
        assert_ne!(first.evidence_id, build.evidence_id);
        assert_eq!(build.request_revision, 3);
        assert!(admission.recorded);
        assert!(!repeated_admission.recorded);
        assert_eq!(admission.evidence_id, repeated_admission.evidence_id);
        assert_ne!(build.evidence_id, admission.evidence_id);
        assert_eq!(admission.request_revision, 4);
        assert_eq!(repeated_admission.request_revision, 4);
        let request_revision = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT revision FROM registry_publish_requests WHERE id = 'request-1'".to_string(),
            ))
            .await
            .expect("publish request query")
            .expect("publish request");
        assert_eq!(
            request_revision
                .try_get::<i64>("", "revision")
                .expect("publish request revision"),
            4
        );
        let evidence = database
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT authority, subject_digest_sha256 FROM registry_publication_evidence"
                    .to_string(),
            ))
            .await
            .expect("evidence query");
        assert_eq!(evidence.len(), 3);
        let authorities = evidence
            .iter()
            .map(|row| row.try_get::<String>("", "authority").expect("authority"))
            .collect::<Vec<_>>();
        assert!(authorities.contains(&"author_signature".to_string()));
        assert!(authorities.contains(&"build_service_attestation".to_string()));
        assert!(authorities.contains(&"platform_admission".to_string()));
        for row in evidence {
            assert_eq!(
                row.try_get::<String>("", "subject_digest_sha256")
                    .expect("subject digest"),
                "a".repeat(64)
            );
        }
    }

