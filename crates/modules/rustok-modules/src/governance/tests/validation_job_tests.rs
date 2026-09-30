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


    #[test]
    fn validation_job_enqueue_requires_request_and_structured_actor() {
        assert!(matches!(
            ModuleValidationJobEnqueueCommand {
                request_id: " ".to_string(),
                expected_revision: 1,
                context: ModuleCommandContext {
                    actor_id: Uuid::new_v4(),
                    tenant_id: None,
                    trace_id: "test:validation-job-enqueue".to_string(),
                    correlation_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                },
                actor_principal: serde_json::json!({ "kind": "user", "id": "operator" }),
                allow_rejected_retry: false,
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand)
        ));
        assert!(matches!(
            ModuleValidationJobEnqueueCommand {
                request_id: "request-1".to_string(),
                expected_revision: 1,
                context: ModuleCommandContext {
                    actor_id: Uuid::new_v4(),
                    tenant_id: None,
                    trace_id: "test:validation-job-enqueue".to_string(),
                    correlation_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                },
                actor_principal: serde_json::json!("operator"),
                allow_rejected_retry: false,
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand)
        ));
        let actor_id = Uuid::new_v4();
        assert!(matches!(
            ModuleValidationJobEnqueueCommand {
                request_id: "request-1".to_string(),
                expected_revision: 1,
                context: ModuleCommandContext {
                    actor_id,
                    tenant_id: Some(Uuid::new_v4()),
                    trace_id: "test:validation-job-enqueue".to_string(),
                    correlation_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                },
                actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
                allow_rejected_retry: false,
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobEnqueueCommand)
        ));
    }


    #[test]
    fn validation_job_result_requires_coherent_evidence() {
        let actor_principal = serde_json::json!({ "kind": "service", "id": "worker" });
        assert!(matches!(
            ModuleValidationJobResultCommand {
                validation_job_id: "job-1".to_string(),
                expected_request_revision: 1,
                actor_principal: actor_principal.clone(),
                outcome: ModuleValidationJobResultOutcome::Passed,
                warnings: Vec::new(),
                errors: vec!["unexpected error".to_string()],
                automated_checks: Vec::new(),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
        ));
        assert!(matches!(
            ModuleValidationJobResultCommand {
                validation_job_id: "job-1".to_string(),
                expected_request_revision: 1,
                actor_principal,
                outcome: ModuleValidationJobResultOutcome::Failed,
                warnings: Vec::new(),
                errors: Vec::new(),
                automated_checks: Vec::new(),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
        ));
    }


    #[test]
    fn validation_job_result_rejects_invalid_automated_check_evidence() {
        let base = ModuleValidationJobResultCommand {
            validation_job_id: "job-1".to_string(),
            expected_request_revision: 1,
            actor_principal: serde_json::json!({ "kind": "service", "id": "worker" }),
            outcome: ModuleValidationJobResultOutcome::Passed,
            warnings: Vec::new(),
            errors: Vec::new(),
            automated_checks: vec![ModuleGovernanceAutomatedCheck {
                key: "artifact_contract".to_string(),
                status: "passed".to_string(),
                detail: Some("Artifact contract validation passed.".to_string()),
            }],
        };
        assert!(base.validate().is_ok());

        let mut blank_key = base.clone();
        blank_key.automated_checks[0].key = " ".to_string();
        assert_eq!(
            blank_key.validate(),
            Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
        );

        let mut duplicate = base;
        duplicate
            .automated_checks
            .push(ModuleGovernanceAutomatedCheck {
                key: "ARTIFACT_CONTRACT".to_string(),
                status: "passed".to_string(),
                detail: None,
            });
        assert_eq!(
            duplicate.validate(),
            Err(ModuleGovernanceError::InvalidValidationJobResultCommand)
        );
    }


    #[test]
    fn governance_event_payload_exposes_only_valid_typed_automated_checks() {
        let payload = governance_event_payload(&serde_json::json!({
            "automated_checks": [
                {
                    "key": " artifact_contract ",
                    "status": " passed ",
                    "detail": " Artifact contract validation passed. "
                },
                {
                    "key": "platform_publication_evidence",
                    "status": "failed",
                    "detail": " "
                },
                {"unknown": "missing_key", "status": "failed"},
                {"key": "", "status": "failed"}
            ]
        }));

        assert_eq!(payload.automated_checks.len(), 2);
        assert_eq!(payload.automated_checks[0].key, "artifact_contract");
        assert_eq!(payload.automated_checks[0].status, "passed");
        assert_eq!(
            payload.automated_checks[0].detail.as_deref(),
            Some("Artifact contract validation passed.")
        );
        assert_eq!(
            payload.automated_checks[1].key,
            "platform_publication_evidence"
        );
        assert!(payload.automated_checks[1].detail.is_none());
    }


    #[tokio::test]
    async fn validation_job_enqueue_replays_only_the_exact_command_context() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, artifact_origin TEXT NOT NULL, validation_errors TEXT NOT NULL,\
                rejected_by_principal TEXT NULL, rejection_reason TEXT NULL, validated_at TEXT NULL,\
                updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_validation_jobs (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, triggered_by TEXT NOT NULL, queue_reason TEXT NOT NULL,\
                attempt_number INTEGER NOT NULL, started_at TEXT NULL, finished_at TEXT NULL,\
                last_error TEXT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_validation_job_enqueue_operations (\
                operation_id TEXT PRIMARY KEY, request_id TEXT NOT NULL, idempotency_key TEXT NOT NULL,\
                expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL, trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                allow_rejected_retry INTEGER NOT NULL, request_status TEXT NOT NULL, queued INTEGER NOT NULL,\
                validation_job_id TEXT NULL, committed_at TEXT NOT NULL,\
                UNIQUE (request_id, idempotency_key)\
             )",
            "INSERT INTO registry_publish_requests (\
                id, revision, slug, version, status, artifact_origin, validation_errors, updated_at\
             ) VALUES (\
                'request-1', 1, 'sample_module', '1.0.0', 'submitted', 'platform_built', '[]', datetime('now')\
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

        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let actor_id = Uuid::new_v4();
        let command = ModuleValidationJobEnqueueCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: "test:validation-job-enqueue-replay".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
            allow_rejected_retry: false,
        };

        let queued = service
            .enqueue_validation_job(command.clone())
            .await
            .expect("initial enqueue");
        assert!(queued.queued);
        assert_eq!(queued.request_status, "validating");
        assert!(queued.validation_job_id.is_some());
        assert_eq!(
            service
                .enqueue_validation_job(command.clone())
                .await
                .expect("exact replay"),
            queued
        );

        let mut changed_context = command;
        changed_context.context.trace_id = "test:changed-validation-job-enqueue-replay".to_string();
        assert_eq!(
            service.enqueue_validation_job(changed_context).await,
            Err(ModuleGovernanceError::ValidationJobEnqueueIdempotencyConflict)
        );

        let job_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_validation_jobs".to_string(),
            ))
            .await
            .expect("job count query")
            .expect("job count row")
            .try_get::<i64>("", "count")
            .expect("job count");
        let event_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_governance_events".to_string(),
            ))
            .await
            .expect("event count query")
            .expect("event count row")
            .try_get::<i64>("", "count")
            .expect("event count");
        assert_eq!(job_count, 1);
        assert_eq!(event_count, 2);
    }


    #[tokio::test]
    async fn validation_worker_result_uses_claimed_request_revision_and_keeps_terminal_replay() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                artifact_origin TEXT NOT NULL, status TEXT NOT NULL, validation_warnings TEXT NOT NULL,\
                validation_errors TEXT NOT NULL, rejected_by_principal TEXT NULL, rejection_reason TEXT NULL,\
                validated_at TEXT NULL, approved_by_principal TEXT NULL, approved_at TEXT NULL,\
                published_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_validation_jobs (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, status TEXT NOT NULL,\
                attempt_number INTEGER NOT NULL, queue_reason TEXT NOT NULL, finished_at TEXT NULL,\
                last_error TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests (\
                id, revision, slug, version, artifact_origin, status, validation_warnings,\
                validation_errors, updated_at\
             ) VALUES (\
                'request-1', 4, 'sample_module', '1.0.0', 'external_prebuilt', 'validating',\
                '[]', '[]', datetime('now')\
             )",
            "INSERT INTO registry_validation_jobs (\
                id, request_id, status, attempt_number, queue_reason, updated_at\
             ) VALUES (\
                'job-1', 'request-1', 'running', 1, 'initial_validation', datetime('now')\
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

        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let failed_result = ModuleValidationJobResultCommand {
            validation_job_id: "job-1".to_string(),
            expected_request_revision: 4,
            actor_principal: serde_json::json!({ "kind": "service", "id": "worker" }),
            outcome: ModuleValidationJobResultOutcome::Failed,
            warnings: Vec::new(),
            errors: vec!["artifact validation failed".to_string()],
            automated_checks: vec![ModuleGovernanceAutomatedCheck {
                key: "artifact_contract".to_string(),
                status: "failed".to_string(),
                detail: Some("Artifact contract validation failed.".to_string()),
            }],
        };
        let mut stale_result = failed_result.clone();
        stale_result.expected_request_revision = 3;
        assert_eq!(
            service.apply_validation_job_result(stale_result).await,
            Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: 3,
                current: 4,
            })
        );

        assert_eq!(
            service
                .apply_validation_job_result(failed_result.clone())
                .await,
            Ok("request-1".to_string())
        );
        assert_eq!(
            service.apply_validation_job_result(failed_result).await,
            Ok("request-1".to_string())
        );
        let request = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status, revision FROM registry_publish_requests WHERE id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("request query")
            .expect("request row");
        assert_eq!(
            request
                .try_get::<String>("", "status")
                .expect("request status"),
            "rejected"
        );
        assert_eq!(
            request
                .try_get::<i64>("", "revision")
                .expect("request revision"),
            5
        );
        let job = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status FROM registry_validation_jobs WHERE id = 'job-1'".to_string(),
            ))
            .await
            .expect("job query")
            .expect("job row");
        assert_eq!(
            job.try_get::<String>("", "status").expect("job status"),
            "failed"
        );
    }

