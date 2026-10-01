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
    ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildPublicationReceipt, ModuleBuildRequest,
    ModuleBuildResult, ModuleBuildScenario, ModuleBuildSignatureAuthority, ModuleBuildSource,
    ModuleBuildToolchain, ModuleBuildValidationOutcome, ModuleBuildValidationProfile,
    ModuleBuildValidationResult, ModuleBuildWitContract,
};
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::{
    ArtifactBlobStore, ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef,
    ControlPlaneInfrastructure, InMemoryArtifactBlobStore, MODULE_BUILD_COMPONENT_TARGET,
    MODULE_BUILD_PROTOCOL_VERSION, MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION,
    MODULE_BUILD_WIT_WORLD, ModuleArtifactDescriptor, ModuleCommandContext,
    ModuleMarketplaceArtifactOrigin, ModuleMarketplaceArtifactRelease, ModuleMarketplaceEntry,
    ModuleMarketplaceEvidenceKind, ModuleMarketplaceEvidenceReference, TrustEvidenceKind,
    TrustEvidenceReference,
};

#[tokio::test]
async fn external_security_stage_reconciles_exact_owner_evidence_once() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL,\
                artifact_origin TEXT NOT NULL, artifact_checksum_sha256 TEXT NULL, submitted_at TEXT NULL\
             )",
        "CREATE TABLE registry_publish_external_staging (\
                request_id TEXT NOT NULL, artifact_digest TEXT NOT NULL, staged_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_publication_evidence (\
                request_id TEXT NOT NULL, authority TEXT NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, signature_digest_sha256 TEXT NULL, created_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_validation_stages (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                stage_key TEXT NOT NULL, status TEXT NOT NULL, triggered_by TEXT NOT NULL,\
                queue_reason TEXT NOT NULL, attempt_number INTEGER NOT NULL, detail TEXT NOT NULL,\
                started_at TEXT NULL, finished_at TEXT NULL, last_error TEXT NULL, claim_id TEXT NULL,\
                claimed_by TEXT NULL, claim_expires_at TEXT NULL, last_heartbeat_at TEXT NULL,\
                runner_kind TEXT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        "INSERT INTO registry_publish_requests \
                (id, slug, version, status, artifact_origin, artifact_checksum_sha256, submitted_at) \
             VALUES ('request-1', 'sample-module', '1.0.0', 'approved', 'external_prebuilt', \
                     'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                     '2026-07-20 10:00:00')",
        "INSERT INTO registry_publish_external_staging (request_id, artifact_digest, staged_at) \
             VALUES ('request-1', \
                     'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                     '2026-07-20 10:01:00')",
        "INSERT INTO registry_publication_evidence \
                (request_id, authority, subject_digest_sha256, signature_digest_sha256, created_at) VALUES \
                ('request-1', 'author_signature', \
                 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                 '2026-07-20 10:02:00'), \
                ('request-1', 'platform_admission', \
                 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 NULL, \
                 '2026-07-20 10:03:00')",
        "INSERT INTO registry_validation_stages \
                (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, \
                 attempt_number, detail, runner_kind, created_at, updated_at) \
             VALUES ('stage-1', 'request-1', 'sample-module', '1.0.0', \
                     'security_policy_review', 'queued', 'validator', 'validation_passed', 1, \
                     'Awaiting owner evidence.', 'owner_evidence', \
                     '2026-07-20 10:04:00', '2026-07-20 10:04:00')",
    ] {
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                statement.to_string(),
            ))
            .await
            .expect("schema or fixture");
    }

    let transaction = database.begin().await.expect("transaction");
    let actor = serde_json::json!({ "kind": "service", "id": "verification-worker" });
    let infrastructure = ControlPlaneInfrastructure::default();
    reconcile_external_prebuilt_security_stage(
        &infrastructure,
        &transaction,
        DbBackend::Sqlite,
        "request-1",
        &actor,
    )
    .await
    .expect("reconcile exact external evidence");
    reconcile_external_prebuilt_security_stage(
        &infrastructure,
        &transaction,
        DbBackend::Sqlite,
        "request-1",
        &actor,
    )
    .await
    .expect("idempotent reconciliation");
    transaction.commit().await.expect("commit");

    let stage = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT status, runner_kind FROM registry_validation_stages WHERE id = 'stage-1'"
                .to_string(),
        ))
        .await
        .expect("stage query")
        .expect("stage");
    assert_eq!(
        stage.try_get::<String>("", "status").expect("status"),
        "passed"
    );
    assert_eq!(
        stage
            .try_get::<Option<String>>("", "runner_kind")
            .expect("runner kind"),
        None
    );
    let events = database
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT event_type FROM registry_governance_events ORDER BY event_type".to_string(),
        ))
        .await
        .expect("events query");
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn alloy_security_stage_reconciles_sandbox_and_admission_evidence_once() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL,\
                artifact_origin TEXT NOT NULL, artifact_checksum_sha256 TEXT NULL, submitted_at TEXT NULL\
             )",
        "CREATE TABLE registry_publish_alloy_staging (\
                request_id TEXT NOT NULL, artifact_digest TEXT NOT NULL, source_digest TEXT NOT NULL,\
                sandbox_execution_id TEXT NOT NULL, sandbox_test_path TEXT NOT NULL,\
                sandbox_executor TEXT NOT NULL, sandbox_scenario_digest TEXT NOT NULL,\
                sandbox_runtime_abi TEXT NOT NULL,\
                sandbox_policy_digest TEXT NOT NULL, sandbox_capability_grants INTEGER NOT NULL,\
                staged_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_publication_evidence (\
                request_id TEXT NOT NULL, authority TEXT NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, signature_digest_sha256 TEXT NULL, created_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_validation_stages (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                stage_key TEXT NOT NULL, status TEXT NOT NULL, triggered_by TEXT NOT NULL,\
                queue_reason TEXT NOT NULL, attempt_number INTEGER NOT NULL, detail TEXT NOT NULL,\
                started_at TEXT NULL, finished_at TEXT NULL, last_error TEXT NULL, claim_id TEXT NULL,\
                claimed_by TEXT NULL, claim_expires_at TEXT NULL, last_heartbeat_at TEXT NULL,\
                runner_kind TEXT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        "INSERT INTO registry_publish_requests \
                (id, slug, version, status, artifact_origin, artifact_checksum_sha256, submitted_at) \
             VALUES ('request-alloy', 'alloy-module', '1.0.0', 'approved', 'alloy_authored', \
                     'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                     '2026-07-20 10:00:00')",
        "INSERT INTO registry_publish_alloy_staging \
                (request_id, artifact_digest, source_digest, sandbox_execution_id, sandbox_test_path, \
                 sandbox_executor, sandbox_scenario_digest, sandbox_runtime_abi, sandbox_policy_digest, \
                 sandbox_capability_grants, staged_at) \
             VALUES ('request-alloy', \
                     'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                     'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                     '11111111-1111-1111-1111-111111111111', \
                     'tests/publication_smoke.rhai', 'rhai', \
                     'sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd', \
                     'rustok:module/runtime@1', \
                     'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', \
                     0, '2026-07-20 10:01:00')",
        "INSERT INTO registry_publication_evidence \
                (request_id, authority, subject_digest_sha256, signature_digest_sha256, created_at) VALUES \
                ('request-alloy', 'author_signature', \
                 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', \
                 '2026-07-20 10:02:00'), \
                ('request-alloy', 'platform_admission', \
                 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                 NULL, \
                 '2026-07-20 10:03:00')",
        "INSERT INTO registry_validation_stages \
                (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, \
                 attempt_number, detail, runner_kind, created_at, updated_at) \
             VALUES ('stage-alloy', 'request-alloy', 'alloy-module', '1.0.0', \
                     'security_policy_review', 'queued', 'validator', 'validation_passed', 1, \
                     'Awaiting owner evidence.', 'owner_evidence', \
                     '2026-07-20 10:04:00', '2026-07-20 10:04:00')",
    ] {
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                statement.to_string(),
            ))
            .await
            .expect("schema or fixture");
    }
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE registry_publish_alloy_staging \
                 SET sandbox_scenario_digest = ?1 WHERE request_id = 'request-alloy'"
                .to_string(),
            vec![alloy_publication_smoke_scenario_digest().into()],
        ))
        .await
        .expect("canonical scenario evidence fixture");

    let transaction = database.begin().await.expect("transaction");
    let actor = serde_json::json!({ "kind": "service", "id": "alloy-release" });
    let infrastructure = ControlPlaneInfrastructure::default();
    for _ in 0..2 {
        reconcile_alloy_authored_security_stage(
            &infrastructure,
            &transaction,
            DbBackend::Sqlite,
            "request-alloy",
            &actor,
        )
        .await
        .expect("reconcile exact Alloy evidence");
    }
    transaction.commit().await.expect("commit");

    let stage = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT status, runner_kind FROM registry_validation_stages \
                 WHERE id = 'stage-alloy'"
                .to_string(),
        ))
        .await
        .expect("stage query")
        .expect("stage");
    assert_eq!(
        stage.try_get::<String>("", "status").expect("status"),
        "passed"
    );
    assert_eq!(
        stage
            .try_get::<Option<String>>("", "runner_kind")
            .expect("runner kind"),
        None
    );
    let events = database
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT event_type FROM registry_governance_events".to_string(),
        ))
        .await
        .expect("events query");
    assert_eq!(events.len(), 2);
}

#[test]
fn validation_stage_report_normalizes_transport_values_inside_owner() {
    let actor_id = Uuid::new_v4();
    let command = ModuleValidationStageReportCommand {
        request_id: " request-1 ".to_string(),
        expected_revision: 1,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id: "test:validation-stage-report".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        stage_key: " TARGETED_TESTS ".to_string(),
        status: " PASSED ".to_string(),
        actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
        reason_code: Some(" TEST_FAILURE ".to_string()),
        requeue: false,
    }
    .normalized()
    .expect("owner normalizes a valid stage report");

    assert_eq!(command.request_id, "request-1");
    assert_eq!(command.stage_key, "targeted_tests");
    assert_eq!(command.status, "passed");
    assert_eq!(command.reason_code.as_deref(), Some("test_failure"));
}

#[test]
fn remote_lease_contract_rejects_blank_identity_and_unknown_reason_code() {
    assert!(matches!(
        ModuleRemoteValidationHeartbeatCommand {
            claim_id: " ".to_string(),
            runner_id: "runner-1".to_string(),
            lease_ttl_ms: 1,
        }
        .validate(),
        Err(ModuleGovernanceError::InvalidRemoteValidationLeaseCommand)
    ));
    assert!(matches!(
        ModuleRemoteValidationTerminalCommand {
            claim_id: "claim-1".to_string(),
            runner_id: "runner-1".to_string(),
            expected_request_revision: 1,
            outcome: ModuleRemoteValidationTerminalOutcome::Passed,
            detail: None,
            reason_code: Some("unknown".to_string()),
        }
        .validate(),
        Err(ModuleGovernanceError::InvalidValidationStageReasonCode(_))
    ));
}

#[tokio::test]
async fn remote_validation_runner_snapshot_counts_only_running_remote_claims() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_validation_stages (\
                id TEXT PRIMARY KEY, status TEXT NOT NULL, runner_kind TEXT NULL, \
                claim_expires_at TEXT NULL\
             )",
        "INSERT INTO registry_validation_stages \
             (id, status, runner_kind, claim_expires_at) VALUES \
             ('remote-expired', 'running', 'remote', '2000-01-01T00:00:00Z'), \
             ('remote-active', 'running', 'remote', '2999-01-01T00:00:00Z'), \
             ('remote-without-expiry', 'running', 'remote', NULL), \
             ('remote-terminal', 'passed', 'remote', '2000-01-01T00:00:00Z'), \
             ('owner-expired', 'running', 'owner_evidence', '2000-01-01T00:00:00Z')",
    ] {
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                statement.to_string(),
            ))
            .await
            .expect("schema or fixture");
    }

    let snapshot = SeaOrmModuleGovernanceService::new(database)
        .remote_validation_runner_snapshot()
        .await
        .expect("owner snapshot");
    assert_eq!(snapshot.active_claims, 3);
    assert_eq!(snapshot.expired_claims, 1);
}

#[tokio::test]
async fn remote_validation_claim_and_terminal_result_advance_the_request_revision() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, crate_name TEXT NOT NULL, artifact_storage_key TEXT NULL,\
                artifact_checksum_sha256 TEXT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
             )",
        "CREATE TABLE registry_validation_stages (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                stage_key TEXT NOT NULL, status TEXT NOT NULL, triggered_by TEXT NOT NULL,\
                queue_reason TEXT NOT NULL, attempt_number INTEGER NOT NULL, detail TEXT NOT NULL,\
                started_at TEXT NULL, finished_at TEXT NULL, last_error TEXT NULL, claim_id TEXT NULL,\
                claimed_by TEXT NULL, claim_expires_at TEXT NULL, last_heartbeat_at TEXT NULL,\
                runner_kind TEXT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
             )",
        "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        "INSERT INTO registry_publish_requests \
             (id, revision, slug, version, status, crate_name, artifact_storage_key, artifact_checksum_sha256) \
             VALUES ('request-1', 5, 'sample_module', '1.0.0', 'approved', 'sample_module', \
                     'artifacts/sha256/sample', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
        "INSERT INTO registry_validation_stages \
             (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, \
              attempt_number, detail, runner_kind) \
             VALUES ('stage-1', 'request-1', 'sample_module', '1.0.0', 'compile_smoke', 'queued', \
                     'system', 'initial_validation', 1, 'Queued.', 'remote')",
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
    let claim = service
        .claim_remote_validation_stage(ModuleRemoteValidationClaimCommand {
            runner_id: "runner-1".to_string(),
            supported_stages: vec!["compile_smoke".to_string()],
            lease_ttl_ms: 60_000,
        })
        .await
        .expect("claim remote stage")
        .expect("remote claim");
    assert_eq!(claim.request_revision, 6);

    let stale_result = service
        .complete_remote_validation_stage(ModuleRemoteValidationTerminalCommand {
            claim_id: claim.claim_id.clone(),
            runner_id: "runner-1".to_string(),
            expected_request_revision: 5,
            outcome: ModuleRemoteValidationTerminalOutcome::Passed,
            detail: None,
            reason_code: Some("local_runner_passed".to_string()),
        })
        .await;
    assert!(matches!(
        stale_result,
        Err(ModuleGovernanceError::PublishRequestRevisionConflict {
            expected: 5,
            current: 6,
        })
    ));

    let terminal = service
        .complete_remote_validation_stage(ModuleRemoteValidationTerminalCommand {
            claim_id: claim.claim_id,
            runner_id: "runner-1".to_string(),
            expected_request_revision: 6,
            outcome: ModuleRemoteValidationTerminalOutcome::Passed,
            detail: None,
            reason_code: Some("local_runner_passed".to_string()),
        })
        .await
        .expect("complete remote stage");
    assert_eq!(terminal.status, "passed");

    let request = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT revision FROM registry_publish_requests WHERE id = 'request-1'".to_string(),
        ))
        .await
        .expect("publish request query")
        .expect("publish request");
    assert_eq!(
        request
            .try_get::<i64>("", "revision")
            .expect("publish request revision"),
        7
    );
}
