use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

use super::fixtures::*;
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


    #[tokio::test]
    async fn external_prebuilt_stage_receipt_rejects_changed_context_or_privilege() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, \
                revision INTEGER NOT NULL, status TEXT NOT NULL, artifact_origin TEXT NOT NULL, \
                artifact_checksum_sha256 TEXT NULL, submitted_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_external_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, expected_revision INTEGER NOT NULL, \
                artifact_digest TEXT NOT NULL, source_evidence_kind TEXT NOT NULL, source_reference TEXT NULL, \
                source_digest TEXT NULL, source_absence_reason TEXT NULL, provenance_reference TEXT NOT NULL, \
                provenance_digest TEXT NOT NULL, provenance_policy_revision TEXT NOT NULL, \
                quarantine_review_reference TEXT NOT NULL, quarantine_policy_revision TEXT NOT NULL, \
                quarantine_approved_by_principal JSON NOT NULL, staged_by_principal JSON NOT NULL, \
                actor_id TEXT NOT NULL, trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, \
                actor_can_manage_modules BOOLEAN NOT NULL, idempotency_key TEXT NOT NULL, \
                staged_at TEXT NOT NULL, UNIQUE (request_id, idempotency_key)\
             )",
            "CREATE TABLE registry_publication_evidence (\
                request_id TEXT NOT NULL, authority TEXT NOT NULL, \
                subject_digest_sha256 TEXT NOT NULL, signature_digest_sha256 TEXT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL, \
                event_type TEXT NOT NULL, actor_principal JSON NOT NULL, publisher_principal JSON NULL, \
                details JSON NOT NULL, created_at TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("stage fixture schema");
        }

        let actor_id = Uuid::new_v4();
        let actor_principal = serde_json::json!({ "kind": "user", "id": actor_id });
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests (\
                    id, slug, version, revision, status, artifact_origin, artifact_checksum_sha256, \
                    submitted_at, updated_at\
                 ) VALUES ('request-1', 'sample_module', '1.0.0', 1, 'submitted', \
                    'external_prebuilt', ?1, NULL, datetime('now'))"
                    .to_string(),
                vec!["a".repeat(64).into()],
            ))
            .await
            .expect("publish request fixture");

        let command = ModuleExternalPrebuiltStageCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: "test:external-prebuilt-stage-receipt".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            source_evidence: ModuleExternalSourceEvidence::Reproducible {
                reference: "https://source.example/modules/sample_module.tar.gz".to_string(),
                digest: format!("sha256:{}", "b".repeat(64)),
            },
            provenance_reference: "https://evidence.example/provenance.json".to_string(),
            provenance_digest: format!("sha256:{}", "c".repeat(64)),
            provenance_policy_revision: "external-provenance-policy".to_string(),
            quarantine_review_reference: "https://reviews.example/quarantine/1".to_string(),
            quarantine_policy_revision: "external-quarantine-policy".to_string(),
            quarantine_approved_by_principal: actor_principal.clone(),
            actor_principal: actor_principal.clone(),
            actor_can_manage_modules: true,
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let first = service
            .stage_external_prebuilt(command.clone())
            .await
            .expect("first stage");
        assert!(first.created);
        let receipt = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT expected_revision, actor_id, trace_id, correlation_id, \
                 actor_can_manage_modules, idempotency_key FROM registry_publish_external_staging"
                    .to_string(),
            ))
            .await
            .expect("stage receipt query")
            .expect("stage receipt");
        assert_eq!(
            receipt
                .try_get::<i64>("", "expected_revision")
                .expect("expected revision"),
            command.expected_revision
        );
        assert_eq!(
            receipt.try_get::<String>("", "actor_id").expect("actor"),
            actor_id.to_string()
        );
        assert_eq!(
            receipt.try_get::<String>("", "trace_id").expect("trace"),
            command.context.trace_id
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "correlation_id")
                .expect("correlation"),
            command.context.correlation_id.to_string()
        );
        assert!(
            receipt
                .try_get::<bool>("", "actor_can_manage_modules")
                .expect("privilege")
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "idempotency_key")
                .expect("idempotency"),
            command.context.idempotency_key.to_string()
        );

        let replay = service
            .stage_external_prebuilt(command.clone())
            .await
            .expect("exact replay");
        assert!(!replay.created);
        assert_eq!(replay.staging_id, first.staging_id);

        let mut changed_trace = command.clone();
        changed_trace.context.trace_id = "test:changed-trace".to_string();
        assert_eq!(
            service.stage_external_prebuilt(changed_trace).await,
            Err(ModuleGovernanceError::ExternalPrebuiltStageIdempotencyConflict)
        );

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_external_staging SET actor_can_manage_modules = 0"
                    .to_string(),
            ))
            .await
            .expect("corrupt privilege receipt");
        assert_eq!(
            service.stage_external_prebuilt(command).await,
            Err(ModuleGovernanceError::ExternalPrebuiltStageIdempotencyConflict)
        );
    }


    #[tokio::test]
    async fn alloy_publication_source_requires_the_exact_owner_receipted_descriptor() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, \
                revision INTEGER NOT NULL, status TEXT NOT NULL, artifact_origin TEXT NOT NULL, \
                license TEXT NOT NULL, artifact_checksum_sha256 TEXT NOT NULL, submitted_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_alloy_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, alloy_tenant_id TEXT NOT NULL, \
                alloy_script_id TEXT NOT NULL, artifact_digest TEXT NOT NULL, source_digest TEXT NOT NULL, \
                source_revision INTEGER NOT NULL, descriptor JSON NOT NULL, descriptor_digest TEXT NOT NULL, \
                review_digest TEXT NOT NULL, staged_at TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("source fixture schema");
        }
        let descriptor = alloy_descriptor("sample_module", "1.0.0", 'a');
        let tenant_id = Uuid::new_v4();
        let script_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests (\
                    id, slug, version, revision, status, artifact_origin, license, \
                    artifact_checksum_sha256, submitted_at\
                 ) VALUES ('request-1', 'sample_module', '1.0.0', 4, 'validating', \
                    'alloy_authored', 'MIT', ?1, '2026-08-01T00:00:00Z')"
                    .to_string(),
                vec!["a".repeat(64).into()],
            ))
            .await
            .expect("publish request fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_alloy_staging (\
                    id, request_id, alloy_tenant_id, alloy_script_id, artifact_digest, source_digest, \
                    source_revision, descriptor, descriptor_digest, review_digest, staged_at\
                 ) VALUES ('stage-1', 'request-1', ?1, ?2, ?3, ?3, 7, ?4, ?5, ?6, \
                    '2026-08-01T00:00:01Z')"
                    .to_string(),
                vec![
                    tenant_id.to_string().into(),
                    script_id.to_string().into(),
                    descriptor.artifact_digest.clone().into(),
                    serde_json::to_string(&descriptor)
                        .expect("descriptor JSON")
                        .into(),
                    crate::canonical_artifact_descriptor_digest(&descriptor).into(),
                    format!("sha256:{}", "b".repeat(64)).into(),
                ],
            ))
            .await
            .expect("Alloy stage fixture");

        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let source = service
            .load_alloy_publication_source("request-1")
            .await
            .expect("owner source");
        assert_eq!(source.request_revision, 4);
        assert_eq!(source.alloy_tenant_id, tenant_id);
        assert_eq!(source.alloy_script_id, script_id);
        assert_eq!(source.source_revision, 7);
        assert_eq!(source.source_digest, descriptor.artifact_digest);
        assert_eq!(source.descriptor, descriptor);
        assert!(source.trust_provenance().validate());

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                format!(
                    "UPDATE registry_publish_alloy_staging SET descriptor_digest = 'sha256:{}'",
                    "c".repeat(64)
                ),
            ))
            .await
            .expect("corrupt descriptor receipt");
        assert_eq!(
            service.load_alloy_publication_source("request-1").await,
            Err(ModuleGovernanceError::AlloyPublicationEvidenceSourceUnavailable)
        );
    }


    #[tokio::test]
    async fn alloy_authored_stage_receipt_is_authority_scoped_and_idempotent() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, \
                revision INTEGER NOT NULL, status TEXT NOT NULL, artifact_origin TEXT NOT NULL, \
                requested_by_principal JSON NOT NULL, publisher_principal JSON NULL, \
                artifact_checksum_sha256 TEXT NULL, submitted_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_alloy_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, expected_revision INTEGER NOT NULL, \
                alloy_tenant_id TEXT NOT NULL, alloy_script_id TEXT NOT NULL, artifact_digest TEXT NOT NULL, \
                source_digest TEXT NOT NULL, source_revision INTEGER NOT NULL, \
                descriptor JSON NOT NULL, descriptor_digest TEXT NOT NULL, parent_release_slug TEXT NULL, \
                parent_release_version TEXT NULL, parent_release_digest TEXT NULL, review_reference TEXT NOT NULL, \
                review_digest TEXT NOT NULL, review_policy_revision TEXT NOT NULL, reviewed_by_principal JSON NOT NULL, \
                sandbox_execution_id TEXT NOT NULL, sandbox_test_path TEXT NOT NULL, sandbox_executor TEXT NOT NULL, \
                sandbox_scenario_digest TEXT NOT NULL, \
                sandbox_runtime_abi TEXT NOT NULL, sandbox_policy_digest TEXT NOT NULL, \
                sandbox_capability_grants INTEGER NOT NULL, staged_by_principal JSON NOT NULL, actor_id TEXT NOT NULL, \
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, \
                staged_at TEXT NOT NULL, UNIQUE (request_id, idempotency_key)\
             )",
            "CREATE TABLE registry_publication_evidence (\
                request_id TEXT NOT NULL, authority TEXT NOT NULL, \
                subject_digest_sha256 TEXT NOT NULL, signature_digest_sha256 TEXT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL, \
                event_type TEXT NOT NULL, actor_principal JSON NOT NULL, publisher_principal JSON NULL, \
                details JSON NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY, owner_principal JSON NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("stage fixture schema");
        }

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests (\
                    id, slug, version, revision, status, artifact_origin, artifact_checksum_sha256, \
                    requested_by_principal, publisher_principal, \
                    submitted_at, updated_at\
                 ) VALUES ('request-1', 'sample_module', '1.0.0', 1, 'submitted', \
                    'alloy_authored', ?1, ?2, ?2, NULL, datetime('now'))"
                    .to_string(),
                vec![
                    "a".repeat(64).into(),
                    serde_json::json!({ "kind": "user", "id": actor_id })
                        .to_string()
                        .into(),
                ],
            ))
            .await
            .expect("publish request fixture");
        let command = ModuleAlloyAuthoredStageCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: Some(tenant_id),
                trace_id: "test:alloy-stage-receipt".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_can_manage_modules: false,
            alloy_tenant_id: tenant_id,
            alloy_script_id: Uuid::new_v4(),
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            source_digest: format!("sha256:{}", "a".repeat(64)),
            source_revision: 1,
            descriptor: alloy_descriptor("sample_module", "1.0.0", 'a'),
            parent_release: None,
            review_reference: "alloy://scripts/example/revisions/1/reviews/approved".to_string(),
            review_digest: format!("sha256:{}", "b".repeat(64)),
            review_policy_revision: "review-policy".to_string(),
            reviewed_by_principal: serde_json::json!({ "kind": "user", "id": Uuid::new_v4() }),
            sandbox_execution_id: Uuid::new_v4(),
            sandbox_test_path: ALLOY_PUBLICATION_SMOKE_TEST_PATH.to_string(),
            sandbox_scenario_digest: alloy_publication_smoke_scenario_digest(),
            sandbox_executor: "rhai".to_string(),
            sandbox_runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
            sandbox_policy_digest: format!("sha256:{}", "c".repeat(64)),
            sandbox_capability_grants: 0,
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let first = service
            .stage_alloy_authored(command.clone())
            .await
            .expect("first stage");
        assert!(first.created);
        let receipt = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT expected_revision, actor_id, trace_id, correlation_id, idempotency_key, sandbox_scenario_digest, \
                        CAST(descriptor AS TEXT) AS descriptor, descriptor_digest \
                 FROM registry_publish_alloy_staging"
                    .to_string(),
            ))
            .await
            .expect("stage receipt query")
            .expect("stage receipt");
        assert_eq!(
            receipt
                .try_get::<i64>("", "expected_revision")
                .expect("expected revision"),
            command.expected_revision
        );
        assert_eq!(
            receipt.try_get::<String>("", "actor_id").expect("actor"),
            actor_id.to_string()
        );
        assert_eq!(
            receipt.try_get::<String>("", "trace_id").expect("trace"),
            command.context.trace_id
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "correlation_id")
                .expect("correlation"),
            command.context.correlation_id.to_string()
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "idempotency_key")
                .expect("idempotency"),
            command.context.idempotency_key.to_string()
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "sandbox_scenario_digest")
                .expect("scenario digest"),
            command.sandbox_scenario_digest
        );
        assert_eq!(
            serde_json::from_str::<ModuleArtifactDescriptor>(
                &receipt
                    .try_get::<String>("", "descriptor")
                    .expect("descriptor")
            )
            .expect("stored descriptor"),
            command.descriptor
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "descriptor_digest")
                .expect("descriptor digest"),
            crate::canonical_artifact_descriptor_digest(&command.descriptor)
        );

        let unauthorized_actor_id = Uuid::new_v4();
        let unauthorized = ModuleAlloyAuthoredStageCommand {
            expected_revision: 2,
            context: ModuleCommandContext {
                actor_id: unauthorized_actor_id,
                tenant_id: Some(tenant_id),
                trace_id: "test:alloy-stage-unauthorized".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_principal: serde_json::json!({ "kind": "user", "id": unauthorized_actor_id }),
            ..command.clone()
        };
        assert_eq!(
            service.stage_alloy_authored(unauthorized).await,
            Err(ModuleGovernanceError::PublishRequestAlloyAuthoredStagingUnauthorized)
        );

        let replay = service
            .stage_alloy_authored(command.clone())
            .await
            .expect("exact replay");
        assert!(!replay.created);
        assert_eq!(replay.staging_id, first.staging_id);

        let mut changed_trace = command.clone();
        changed_trace.context.trace_id = "test:changed-trace".to_string();
        assert_eq!(
            service.stage_alloy_authored(changed_trace).await,
            Err(ModuleGovernanceError::AlloyAuthoredStageIdempotencyConflict)
        );

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_alloy_staging SET actor_id = 'corrupt-actor'".to_string(),
            ))
            .await
            .expect("corrupt actor receipt");
        assert_eq!(
            service.stage_alloy_authored(command).await,
            Err(ModuleGovernanceError::AlloyAuthoredStageIdempotencyConflict)
        );
    }


    #[test]
    fn alloy_stage_requires_fixed_zero_grant_sandbox_evidence() {
        assert_eq!(
            alloy_publication_smoke_scenario_digest(),
            "sha256:dcbfb4014f6a0078405a5caf748d39f4df74a96f1f7b7dc9c3f901ec37d6f1bb"
        );

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut command = ModuleAlloyAuthoredStageCommand {
            request_id: "request-alloy".into(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: Some(tenant_id),
                trace_id: "test:alloy-stage".into(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_can_manage_modules: true,
            alloy_tenant_id: tenant_id,
            alloy_script_id: Uuid::new_v4(),
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            source_digest: format!("sha256:{}", "a".repeat(64)),
            source_revision: 1,
            descriptor: alloy_descriptor("alloy_module", "1.1.0", 'a'),
            parent_release: None,
            review_reference: "alloy://scripts/example/reviews/approved".into(),
            review_digest: format!("sha256:{}", "b".repeat(64)),
            review_policy_revision: "alloy-review-v1".into(),
            reviewed_by_principal: serde_json::json!({ "kind": "user", "id": "reviewer" }),
            sandbox_execution_id: Uuid::new_v4(),
            sandbox_test_path: ALLOY_PUBLICATION_SMOKE_TEST_PATH.into(),
            sandbox_scenario_digest: alloy_publication_smoke_scenario_digest(),
            sandbox_executor: "rhai".into(),
            sandbox_runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.into(),
            sandbox_policy_digest: format!("sha256:{}", "c".repeat(64)),
            sandbox_capability_grants: 0,
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
        };
        assert!(command.validate().is_ok());

        command.sandbox_capability_grants = 1;
        assert_eq!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)
        );
        command.sandbox_capability_grants = 0;
        command.sandbox_test_path = "tests/other.rhai".into();
        assert_eq!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)
        );
        command.sandbox_test_path = ALLOY_PUBLICATION_SMOKE_TEST_PATH.into();
        command.sandbox_scenario_digest = format!("sha256:{}", "d".repeat(64));
        assert_eq!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)
        );
        command.sandbox_scenario_digest = alloy_publication_smoke_scenario_digest();
        command.parent_release = Some(ArtifactReleaseRef {
            slug: "alloy_module".into(),
            version: "1.0.0".into(),
            digest: "sha256:invalid".into(),
        });
        assert_eq!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidAlloyAuthoredStageCommand)
        );
    }


    #[test]
    fn external_prebuilt_stage_requires_explicit_source_evidence() {
        let actor_id = Uuid::new_v4();
        let command = ModuleExternalPrebuiltStageCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: "test:external-prebuilt-stage".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            source_evidence: ModuleExternalSourceEvidence::Unavailable {
                reason_code: "source_unavailable".to_string(),
            },
            provenance_reference: "https://evidence.example/provenance.json".to_string(),
            provenance_digest: format!("sha256:{}", "b".repeat(64)),
            provenance_policy_revision: "external-provenance-v1".to_string(),
            quarantine_review_reference: "https://reviews.example/quarantine/1".to_string(),
            quarantine_policy_revision: "external-quarantine-v1".to_string(),
            quarantine_approved_by_principal: serde_json::json!({
                "kind": "user",
                "id": actor_id,
            }),
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
            actor_can_manage_modules: true,
        };
        assert!(command.validate().is_ok());

        let mut tenant_scoped = command.clone();
        tenant_scoped.context.tenant_id = Some(Uuid::new_v4());
        assert!(matches!(
            tenant_scoped.validate(),
            Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand)
        ));

        let mut unclassified_absence = command;
        unclassified_absence.source_evidence = ModuleExternalSourceEvidence::Unavailable {
            reason_code: "not_recorded".to_string(),
        };
        assert!(matches!(
            unclassified_absence.validate(),
            Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand)
        ));
    }


    #[tokio::test]
    async fn owner_rejects_external_prebuilt_staging_without_operator_quarantine_authority() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let service = SeaOrmModuleGovernanceService::new(database);
        let actor_id = Uuid::new_v4();
        let command = ModuleExternalPrebuiltStageCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: "test:external-prebuilt-authority".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            source_evidence: ModuleExternalSourceEvidence::Unavailable {
                reason_code: "source_unavailable".to_string(),
            },
            provenance_reference: "https://evidence.example/provenance.json".to_string(),
            provenance_digest: format!("sha256:{}", "b".repeat(64)),
            provenance_policy_revision: "external-provenance-policy".to_string(),
            quarantine_review_reference: "https://reviews.example/quarantine/1".to_string(),
            quarantine_policy_revision: "external-quarantine-policy".to_string(),
            quarantine_approved_by_principal: serde_json::json!({
                "kind": "user",
                "id": actor_id,
            }),
            actor_principal: serde_json::json!({
                "kind": "user",
                "id": actor_id,
            }),
            actor_can_manage_modules: false,
        };
        assert_eq!(
            service.stage_external_prebuilt(command.clone()).await,
            Err(ModuleGovernanceError::PublishRequestExternalPrebuiltStagingUnauthorized)
        );

        let mut mismatched_approver = command;
        mismatched_approver.actor_can_manage_modules = true;
        mismatched_approver.quarantine_approved_by_principal =
            serde_json::json!({ "kind": "user", "id": "other-operator" });
        assert_eq!(
            service.stage_external_prebuilt(mismatched_approver).await,
            Err(ModuleGovernanceError::InvalidExternalPrebuiltStageCommand)
        );
    }

