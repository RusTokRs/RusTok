#![allow(unused_imports)]

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
    ModuleMarketplaceEntry, ModuleMarketplaceEvidenceKind, ModuleMarketplaceEvidenceReference,
    TrustEvidenceKind, TrustEvidenceReference, MODULE_BUILD_COMPONENT_TARGET,
    MODULE_BUILD_PROTOCOL_VERSION, MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION,
    MODULE_BUILD_WIT_WORLD,
};


    #[tokio::test]
    async fn alloy_validation_claim_carries_the_exact_receipted_descriptor() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, revision INTEGER NOT NULL, \
                crate_name TEXT NOT NULL, ownership TEXT NOT NULL, trust_level TEXT NOT NULL, license TEXT NOT NULL, \
                entry_type TEXT NULL, artifact_origin TEXT NOT NULL, marketplace JSON NOT NULL, ui_packages JSON NOT NULL, \
                validation_warnings JSON NOT NULL, status TEXT NOT NULL, artifact_storage_key TEXT NULL, \
                artifact_checksum_sha256 TEXT NULL, artifact_size INTEGER NULL, artifact_content_type TEXT NULL, \
                default_locale TEXT NOT NULL, submitted_at TEXT NULL, validation_errors JSON NOT NULL, \
                rejected_by_principal JSON NULL, rejection_reason TEXT NULL, validated_at TEXT NULL, \
                approved_by_principal JSON NULL, approved_at TEXT NULL, published_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_validation_jobs (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, status TEXT NOT NULL, attempt_number INTEGER NOT NULL, \
                queue_reason TEXT NOT NULL, started_at TEXT NULL, finished_at TEXT NULL, last_error TEXT NULL, \
                updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_request_translations (\
                request_id TEXT NOT NULL, locale TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_alloy_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, artifact_digest TEXT NOT NULL, source_digest TEXT NOT NULL, \
                descriptor JSON NOT NULL, descriptor_digest TEXT NOT NULL, staged_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL, event_type TEXT NOT NULL, \
                actor_principal JSON NOT NULL, publisher_principal JSON NULL, details JSON NOT NULL, created_at TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("claim fixture schema");
        }
        let descriptor = alloy_descriptor("sample_module", "1.0.0", 'a');
        let descriptor_digest = crate::canonical_artifact_descriptor_digest(&descriptor);
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests (\
                    id, slug, version, revision, crate_name, ownership, trust_level, license, entry_type, \
                    artifact_origin, marketplace, ui_packages, validation_warnings, status, artifact_storage_key, \
                    artifact_checksum_sha256, artifact_size, artifact_content_type, default_locale, submitted_at, \
                    validation_errors, updated_at\
                 ) VALUES (\
                    'request-alloy-validation', 'sample_module', '1.0.0', 3, 'sample_module', 'first_party', \
                    'sandboxed', 'MIT', NULL, 'alloy_authored', '{}', '{\"admin\":null,\"storefront\":null}', '[]', \
                    'validating', 'registry-publish-artifact/sha256/a', ?, 128, ?, 'en-US', datetime('now'), '[]', datetime('now')\
                 )"
                    .to_string(),
                vec![
                    "a".repeat(64).into(),
                    rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE.into(),
                ],
            ))
            .await
            .expect("publish request fixture");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_validation_jobs (\
                    id, request_id, status, attempt_number, queue_reason, started_at, finished_at, last_error, updated_at\
                 ) VALUES (\
                    'job-alloy-validation', 'request-alloy-validation', 'queued', 1, 'artifact_attached', NULL, NULL, NULL, datetime('now')\
                 )"
                    .to_string(),
            ))
            .await
            .expect("validation job fixture");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_request_translations (request_id, locale, name, description) VALUES (\
                    'request-alloy-validation', 'en-US', 'Sample module', 'Sample module description'\
                 )"
                    .to_string(),
            ))
            .await
            .expect("translation fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_alloy_staging (\
                    id, request_id, artifact_digest, source_digest, descriptor, descriptor_digest, staged_at\
                 ) VALUES ('stage-alloy-validation', 'request-alloy-validation', ?, ?, ?, ?, datetime('now'))"
                    .to_string(),
                vec![
                    descriptor.artifact_digest.clone().into(),
                    descriptor.artifact_digest.clone().into(),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                    descriptor_digest.into(),
                ],
            ))
            .await
            .expect("Alloy stage fixture");

        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let claim = service
            .claim_validation_job(ModuleValidationJobClaimCommand {
                validation_job_id: "job-alloy-validation".to_string(),
                actor_principal: serde_json::json!({ "kind": "service", "id": "worker" }),
            })
            .await
            .expect("claim")
            .expect("claim result");
        let work_item = claim.work_item.expect("immutable work item");

        assert!(claim.should_run);
        assert_eq!(work_item.alloy_descriptor.as_ref(), Some(&descriptor));
        assert_eq!(
            work_item.artifact_origin,
            ModulePublicationArtifactOrigin::AlloyAuthored
        );
        assert_eq!(work_item.expected_request_revision, 3);
    }


    #[tokio::test]
    async fn imported_alloy_fork_stage_and_published_contract_preserve_parent_lineage() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL,\
                artifact_origin TEXT NOT NULL, artifact_checksum_sha256 TEXT NULL, \
                requested_by_principal JSON NOT NULL, publisher_principal JSON NULL, submitted_at TEXT NULL,\
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
             )",
            "CREATE TABLE registry_publish_alloy_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, expected_revision INTEGER NOT NULL, alloy_tenant_id TEXT NOT NULL,\
                alloy_script_id TEXT NOT NULL, artifact_digest TEXT NOT NULL, source_digest TEXT NOT NULL,\
                source_revision INTEGER NOT NULL, descriptor JSON NOT NULL, descriptor_digest TEXT NOT NULL, parent_release_slug TEXT NULL,\
                parent_release_version TEXT NULL, parent_release_digest TEXT NULL,\
                review_reference TEXT NOT NULL, review_digest TEXT NOT NULL,\
                review_policy_revision TEXT NOT NULL, reviewed_by_principal JSON NOT NULL,\
                sandbox_execution_id TEXT NOT NULL, sandbox_test_path TEXT NOT NULL,\
                sandbox_executor TEXT NOT NULL, sandbox_scenario_digest TEXT NOT NULL,\
                sandbox_runtime_abi TEXT NOT NULL,\
                sandbox_policy_digest TEXT NOT NULL, sandbox_capability_grants INTEGER NOT NULL,\
                staged_by_principal JSON NOT NULL, actor_id TEXT NOT NULL, trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, staged_at TEXT NOT NULL,\
                UNIQUE (request_id, idempotency_key)\
             )",
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL, checksum_sha256 TEXT NOT NULL,\
                status TEXT NOT NULL, artifact_origin TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_release_artifacts (\
                release_id TEXT PRIMARY KEY, request_id TEXT NULL, artifact JSON NULL,\
                descriptor JSON NULL, lineage JSON NULL, created_at TEXT NULL\
             )",
            "CREATE TABLE registry_publication_evidence (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, authority TEXT NOT NULL, subject_digest_sha256 TEXT NOT NULL,\
                evidence_reference TEXT NOT NULL, signature_digest_sha256 TEXT NULL, evidence_digest_sha256 TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal JSON NOT NULL, publisher_principal JSON NULL,\
                details JSON NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, registry_id TEXT NOT NULL, repository TEXT NOT NULL,\
                manifest_digest TEXT NOT NULL, payload_digest TEXT NOT NULL, descriptor_digest TEXT NOT NULL,\
                descriptor JSON NOT NULL, runtime_kind TEXT NOT NULL, signature_reference TEXT NOT NULL,\
                signature_digest TEXT NOT NULL, provenance_reference TEXT NOT NULL, provenance_digest TEXT NOT NULL,\
                sbom_reference TEXT NOT NULL, sbom_digest TEXT NOT NULL, admission_reference TEXT NOT NULL,\
                admission_digest TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests \
             (id, slug, version, status, artifact_origin, artifact_checksum_sha256, \
              requested_by_principal, publisher_principal, submitted_at) VALUES \
             ('request-fork', 'tax_rule', '1.1.0', 'submitted', 'alloy_authored', \
              'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
              '{\"kind\":\"user\",\"id\":\"publisher\"}', \
              '{\"kind\":\"user\",\"id\":\"publisher\"}', datetime('now'))",
            "INSERT INTO registry_module_releases \
             (id, request_id, slug, version, checksum_sha256, status, artifact_origin) VALUES \
             ('parent-release', 'parent-request', 'tax_rule', '1.0.0', \
              'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'active', 'alloy_authored')",
            "INSERT INTO registry_module_release_artifacts (release_id) VALUES ('parent-release')",
            "INSERT INTO registry_publish_platform_admissions \
             (request_id, registry_id, repository, manifest_digest, payload_digest, descriptor_digest, \
              descriptor, runtime_kind, signature_reference, signature_digest, provenance_reference, \
              provenance_digest, sbom_reference, sbom_digest, admission_reference, admission_digest) VALUES \
             ('parent-request', 'local', 'modules/tax_rule', \
              'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
              'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
              'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', '{}', 'rhai', \
              'evidence://signature', 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
              'evidence://provenance', 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
              'evidence://sbom', 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
              'evidence://admission', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema or fixture");
        }

        let parent = ArtifactReleaseRef {
            slug: "tax_rule".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let alloy_tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        service
            .stage_alloy_authored(ModuleAlloyAuthoredStageCommand {
                request_id: "request-fork".to_string(),
                expected_revision: 1,
                context: ModuleCommandContext {
                    actor_id,
                    tenant_id: Some(alloy_tenant_id),
                    trace_id: "test:alloy-fork".to_string(),
                    correlation_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                },
                actor_can_manage_modules: true,
                alloy_tenant_id,
                alloy_script_id: Uuid::new_v4(),
                artifact_digest: format!("sha256:{}", "b".repeat(64)),
                source_digest: format!("sha256:{}", "b".repeat(64)),
                source_revision: 2,
                descriptor: alloy_descriptor("tax_rule", "1.1.0", 'b'),
                parent_release: Some(parent.clone()),
                review_reference: "alloy://scripts/fork/revisions/2/reviews/approved".to_string(),
                review_digest: format!("sha256:{}", "c".repeat(64)),
                review_policy_revision: "review-policy".to_string(),
                reviewed_by_principal: serde_json::json!({ "kind": "user", "id": "reviewer" }),
                sandbox_execution_id: Uuid::new_v4(),
                sandbox_test_path: ALLOY_PUBLICATION_SMOKE_TEST_PATH.to_string(),
                sandbox_scenario_digest: alloy_publication_smoke_scenario_digest(),
                sandbox_executor: "rhai".to_string(),
                sandbox_runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
                sandbox_policy_digest: format!("sha256:{}", "d".repeat(64)),
                sandbox_capability_grants: 0,
                actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
            })
            .await
            .expect("stage imported fork");

        let descriptor = ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "tax_rule".to_string(),
            version: "1.1.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", "b".repeat(64)),
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
        };
        descriptor.validate().expect("descriptor");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_platform_admissions \
                 (request_id, registry_id, repository, manifest_digest, payload_digest, descriptor_digest, \
                  descriptor, runtime_kind, signature_reference, signature_digest, provenance_reference, \
                  provenance_digest, sbom_reference, sbom_digest, admission_reference, admission_digest) \
                 VALUES ('request-fork', 'local', 'modules/tax_rule', ?, ?, ?, ?, 'rhai', \
                         'evidence://signature', ?, 'evidence://provenance', ?, 'evidence://sbom', ?, \
                         'evidence://admission', ?)"
                    .to_string(),
                vec![
                    format!("sha256:{}", "f".repeat(64)).into(),
                    format!("sha256:{}", "b".repeat(64)).into(),
                    crate::canonical_artifact_descriptor_digest(&descriptor).into(),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                    format!("sha256:{}", "1".repeat(64)).into(),
                    format!("sha256:{}", "2".repeat(64)).into(),
                    format!("sha256:{}", "3".repeat(64)).into(),
                    "4".repeat(64).into(),
                ],
            ))
            .await
            .expect("admission fixture");
        for (authority, marker) in [("author_signature", '5'), ("marketplace_approval", '6')] {
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO registry_publication_evidence \
                     (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                      signature_digest_sha256, evidence_digest_sha256, created_at) \
                     VALUES (?, 'request-fork', ?, 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                             'evidence://publication', CASE WHEN ? = 'author_signature' THEN ? ELSE NULL END, ?, datetime('now'))"
                        .to_string(),
                    vec![
                        format!("evidence-{authority}").into(),
                        authority.into(),
                        authority.into(),
                        "f".repeat(64).into(),
                        marker.to_string().repeat(64).into(),
                    ],
                ))
                .await
                .expect("publication evidence fixture");
        }

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_module_releases SET status = 'yanked' WHERE id = 'parent-release'"
                    .to_string(),
            ))
            .await
            .expect("yank parent fixture");
        let revoked_parent_transaction =
            database.begin().await.expect("revoked parent transaction");
        assert_eq!(
            canonical_marketplace_artifact_contract(
                &revoked_parent_transaction,
                DbBackend::Sqlite,
                "request-fork",
                "tax_rule",
                "1.1.0",
                ModulePublicationArtifactOrigin::AlloyAuthored,
                &"b".repeat(64),
            )
            .await,
            Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)
        );
        revoked_parent_transaction
            .rollback()
            .await
            .expect("rollback revoked parent transaction");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_module_releases SET status = 'active' WHERE id = 'parent-release'"
                    .to_string(),
            ))
            .await
            .expect("restore parent fixture");

        let transaction = database.begin().await.expect("publication transaction");
        let (artifact, published_descriptor, lineage) = canonical_marketplace_artifact_contract(
            &transaction,
            DbBackend::Sqlite,
            "request-fork",
            "tax_rule",
            "1.1.0",
            ModulePublicationArtifactOrigin::AlloyAuthored,
            &"b".repeat(64),
        )
        .await
        .expect("canonical fork contract");
        persist_published_artifact_contract(
            &transaction,
            DbBackend::Sqlite,
            "child-release",
            "request-fork",
            &artifact,
            &published_descriptor,
            &lineage,
        )
        .await
        .expect("persist fork contract");
        transaction.commit().await.expect("commit fork contract");

        assert_eq!(lineage.parent_release, Some(parent));
        assert_eq!(lineage.origin, crate::ArtifactOrigin::Marketplace);
        assert_eq!(lineage.source_digest, artifact.source_digest);
        let stored = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT CAST(lineage AS TEXT) AS lineage FROM registry_module_release_artifacts \
                 WHERE release_id = 'child-release'"
                    .to_string(),
            ))
            .await
            .expect("stored lineage query")
            .expect("stored lineage row");
        let stored_lineage = serde_json::from_str::<crate::ArtifactSourceLineage>(
            &stored
                .try_get::<String>("", "lineage")
                .expect("stored lineage JSON"),
        )
        .expect("decode stored lineage");
        assert_eq!(stored_lineage, lineage);
    }

