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
async fn release_yank_persists_release_and_audit_fact_together() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                publisher_principal TEXT NOT NULL, status TEXT NOT NULL, yanked_reason TEXT NULL,\
                yanked_by_principal TEXT NULL, yanked_at TEXT NULL, artifact_storage_key TEXT NULL,\
                checksum_sha256 TEXT NULL, artifact_size INTEGER NULL, updated_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_release_yank_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, release_id TEXT NOT NULL,\
                idempotency_key TEXT NOT NULL, actor_id TEXT NOT NULL, trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL, actor_principal TEXT NOT NULL,\
                actor_can_manage_modules INTEGER NOT NULL, reason TEXT NOT NULL,\
                reason_code TEXT NOT NULL, resulting_status TEXT NOT NULL,\
                committed_at TEXT NOT NULL, UNIQUE (release_id, idempotency_key)\
             )",
        "INSERT INTO registry_module_releases (\
                id, request_id, slug, version, publisher_principal, status, artifact_storage_key,\
                checksum_sha256, artifact_size, updated_at\
             ) VALUES (\
                'release-1', 'request-1', 'sample_module', '1.0.0', '{\"subject\":\"publisher\"}', 'active',\
                'registry/sample-1.0.0', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 42, datetime('now')\
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
    let idempotency_key = Uuid::new_v4();
    let command = ModuleReleaseYankCommand {
        slug: "sample_module".to_string(),
        version: "1.0.0".to_string(),
        reason: "critical regression".to_string(),
        reason_code: "critical_regression".to_string(),
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id: format!("test:yank:{idempotency_key}"),
            correlation_id: idempotency_key,
            idempotency_key,
        },
        actor_principal: serde_json::json!({
            "kind": "user",
            "user_id": actor_id,
            "subject": format!("user:{actor_id}"),
        }),
        actor_can_manage_modules: true,
    };
    let service = SeaOrmModuleGovernanceService::new(database.clone());
    service
        .yank_release(command.clone())
        .await
        .expect("yank release");
    service
        .yank_release(command.clone())
        .await
        .expect("exact yank replay");
    let mut changed_replay = command;
    changed_replay.context.trace_id = "test:yank:changed-trace".to_string();
    assert!(matches!(
        service.yank_release(changed_replay).await,
        Err(ModuleGovernanceError::ReleaseYankIdempotencyConflict)
    ));
    let release = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT status, yanked_reason, artifact_storage_key, checksum_sha256, artifact_size \
                 FROM registry_module_releases"
                .to_string(),
        ))
        .await
        .expect("release query")
        .expect("release row");
    assert_eq!(
        release.try_get::<String>("", "status").expect("status"),
        "yanked"
    );
    assert_eq!(
        release
            .try_get::<String>("", "yanked_reason")
            .expect("reason"),
        "critical regression"
    );
    assert_eq!(
        release
            .try_get::<String>("", "artifact_storage_key")
            .expect("artifact storage key"),
        "registry/sample-1.0.0"
    );
    assert_eq!(
        release
            .try_get::<String>("", "checksum_sha256")
            .expect("artifact checksum"),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        release
            .try_get::<i64>("", "artifact_size")
            .expect("artifact size"),
        42
    );
    let event = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT request_id, release_id, event_type FROM registry_governance_events".to_string(),
        ))
        .await
        .expect("event query")
        .expect("event row");
    assert_eq!(
        event.try_get::<String>("", "request_id").expect("request"),
        "request-1"
    );
    assert_eq!(
        event.try_get::<String>("", "release_id").expect("release"),
        "release-1"
    );
    assert_eq!(
        event
            .try_get::<String>("", "event_type")
            .expect("event type"),
        "release_yanked"
    );
}

#[tokio::test]
async fn owner_transfer_persists_binding_and_audit_fact_together() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY, owner_principal TEXT NOT NULL, \
                bound_by_principal TEXT NOT NULL, bound_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_owner_transfer_operations (\
                operation_id TEXT PRIMARY KEY, slug TEXT NOT NULL, idempotency_key TEXT NOT NULL,\
                actor_id TEXT NOT NULL, trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL,\
                previous_owner_principal TEXT NOT NULL, new_owner_principal TEXT NOT NULL,\
                actor_principal TEXT NOT NULL, actor_can_manage_modules INTEGER NOT NULL,\
                reason TEXT NOT NULL, reason_code TEXT NOT NULL, committed_at TEXT NOT NULL,\
                UNIQUE (slug, idempotency_key)\
             )",
        "INSERT INTO registry_module_owners (\
                slug, owner_principal, bound_by_principal, bound_at, updated_at\
             ) VALUES (\
                'sample_module', '{\"kind\":\"user\",\"id\":\"previous\"}', \
                '{\"kind\":\"user\",\"id\":\"operator\"}', datetime('now'), datetime('now')\
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
    let unrelated_actor_id = Uuid::new_v4();
    let operator_actor_id = Uuid::new_v4();
    let service = SeaOrmModuleGovernanceService::new(database.clone());
    assert_eq!(
            service
                .transfer_owner(ModuleOwnerTransferCommand {
                    slug: "sample_module".to_string(),
                    new_owner_principal: serde_json::json!({ "kind": "user", "id": "next" }),
                    context: ModuleCommandContext {
                        actor_id: unrelated_actor_id,
                        tenant_id: None,
                        trace_id: "test:owner-transfer:unauthorized".to_string(),
                        correlation_id: Uuid::new_v4(),
                        idempotency_key: Uuid::new_v4(),
                    },
                    actor_principal: serde_json::json!({ "kind": "user", "user_id": unrelated_actor_id, "subject": format!("user:{unrelated_actor_id}") }),
                    actor_can_manage_modules: false,
                    reason: "maintenance handoff".to_string(),
                    reason_code: "maintenance_handoff".to_string(),
                })
                .await,
            Err(ModuleGovernanceError::OwnerTransferUnauthorized)
        );
    let command = ModuleOwnerTransferCommand {
        slug: "sample_module".to_string(),
        new_owner_principal: serde_json::json!({ "kind": "user", "id": "next" }),
        context: ModuleCommandContext {
            actor_id: operator_actor_id,
            tenant_id: None,
            trace_id: "test:owner-transfer:success".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        actor_principal: serde_json::json!({ "kind": "user", "user_id": operator_actor_id, "subject": format!("user:{operator_actor_id}") }),
        actor_can_manage_modules: true,
        reason: "maintenance handoff".to_string(),
        reason_code: "maintenance_handoff".to_string(),
    };
    service
        .transfer_owner(command.clone())
        .await
        .expect("transfer owner");
    service
        .transfer_owner(command.clone())
        .await
        .expect("exact owner-transfer replay");
    let mut changed_replay = command;
    changed_replay.context.trace_id = "test:owner-transfer:changed-trace".to_string();
    assert!(matches!(
        service.transfer_owner(changed_replay).await,
        Err(ModuleGovernanceError::OwnerTransferIdempotencyConflict)
    ));
    let binding = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT owner_principal, bound_by_principal FROM registry_module_owners".to_string(),
        ))
        .await
        .expect("binding query")
        .expect("binding row");
    let owner: serde_json::Value = serde_json::from_str(
        &binding
            .try_get::<String>("", "owner_principal")
            .expect("owner"),
    )
    .expect("owner JSON");
    assert_eq!(owner["id"], "next");
    let owner_snapshot = SeaOrmModuleGovernanceService::new(database.clone())
        .owner_binding_snapshot("sample_module")
        .await
        .expect("owner binding projection")
        .expect("owner binding");
    assert_eq!(owner_snapshot.owner_principal["id"], "next");
    assert_eq!(
        owner_snapshot.bound_by_principal["user_id"],
        operator_actor_id.to_string()
    );
    let event = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT event_type, request_id, release_id, details FROM registry_governance_events"
                .to_string(),
        ))
        .await
        .expect("event query")
        .expect("event row");
    assert_eq!(
        event
            .try_get::<String>("", "event_type")
            .expect("event type"),
        "owner_transferred"
    );
    assert_eq!(
        event
            .try_get::<Option<String>>("", "request_id")
            .expect("request id"),
        None
    );
    assert_eq!(
        event
            .try_get::<Option<String>>("", "release_id")
            .expect("release id"),
        None
    );
    let details: serde_json::Value = serde_json::from_str(
        &event
            .try_get::<String>("", "details")
            .expect("event details"),
    )
    .expect("details JSON");
    assert_eq!(details["reason_code"], "maintenance_handoff");
    assert_eq!(
        details["owner_transition"]["previous_owner"]["id"],
        "previous"
    );
    assert_eq!(details["owner_transition"]["new_owner"]["id"], "next");
}

#[tokio::test]
async fn hold_then_resume_preserves_predecessor_state_and_audit_facts() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("database");
    for statement in [
        "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL,\
                artifact_origin TEXT NOT NULL,\
                publisher_principal TEXT NULL, held_by_principal TEXT NULL, held_reason TEXT NULL,\
                held_reason_code TEXT NULL, held_at TEXT NULL, held_from_status TEXT NULL, updated_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
        "CREATE TABLE registry_publish_request_review_operations (\
                operation_id TEXT PRIMARY KEY, request_id TEXT NOT NULL, operation_kind TEXT NOT NULL,\
                idempotency_key TEXT NOT NULL, expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal TEXT NOT NULL,\
                reason TEXT NOT NULL, reason_code TEXT NOT NULL, resulting_status TEXT NOT NULL,\
                resulting_revision INTEGER NOT NULL, committed_at TEXT NOT NULL,\
                UNIQUE (request_id, idempotency_key)\
             )",
        "INSERT INTO registry_publish_requests (\
                id, slug, version, status, artifact_origin, publisher_principal, updated_at\
             ) VALUES (\
                'request-1', 'sample_module', '1.0.0', 'approved', 'platform_built', \
                '{\"kind\":\"user\",\"id\":\"publisher\"}', datetime('now')\
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
    let actor_principal = serde_json::json!({
        "kind": "user",
        "user_id": actor_id,
        "subject": format!("user:{actor_id}"),
    });
    assert_eq!(
        service
            .hold_publish_request(ModulePublishRequestHoldCommand {
                request_id: "request-1".to_string(),
                expected_revision: 2,
                context: ModuleCommandContext {
                    actor_id,
                    tenant_id: None,
                    trace_id: "test:review:stale".to_string(),
                    correlation_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                },
                actor_principal: actor_principal.clone(),
                reason: "release window".to_string(),
                reason_code: "release_window".to_string(),
            })
            .await,
        Err(ModuleGovernanceError::PublishRequestRevisionConflict {
            expected: 2,
            current: 1,
        })
    );
    let idempotency_key = Uuid::new_v4();
    let hold = ModulePublishRequestHoldCommand {
        request_id: "request-1".to_string(),
        expected_revision: 1,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id: format!("test:review:{idempotency_key}"),
            correlation_id: idempotency_key,
            idempotency_key,
        },
        actor_principal: actor_principal.clone(),
        reason: "release window".to_string(),
        reason_code: "release_window".to_string(),
    };
    service
        .hold_publish_request(hold.clone())
        .await
        .expect("hold request");
    service
        .hold_publish_request(hold.clone())
        .await
        .expect("hold request replay");
    let mut changed_replay = hold;
    changed_replay.context.trace_id = "test:review:changed-trace".to_string();
    assert!(matches!(
        service.hold_publish_request(changed_replay).await,
        Err(ModuleGovernanceError::PublishRequestReviewIdempotencyConflict)
    ));
    let resume_idempotency_key = Uuid::new_v4();
    service
        .resume_publish_request(ModulePublishRequestResumeCommand {
            request_id: "request-1".to_string(),
            expected_revision: 2,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: format!("test:review:{resume_idempotency_key}"),
                correlation_id: resume_idempotency_key,
                idempotency_key: resume_idempotency_key,
            },
            actor_principal,
            reason: "window closed".to_string(),
            reason_code: "review_complete".to_string(),
        })
        .await
        .expect("resume request");
    let request = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT status, held_from_status FROM registry_publish_requests".to_string(),
        ))
        .await
        .expect("request query")
        .expect("request row");
    assert_eq!(
        request.try_get::<String>("", "status").expect("status"),
        "approved"
    );
    assert_eq!(
        request
            .try_get::<String>("", "held_from_status")
            .expect("held predecessor"),
        "approved"
    );
    let events = database
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT event_type FROM registry_governance_events ORDER BY created_at, id".to_string(),
        ))
        .await
        .expect("event query");
    assert_eq!(events.len(), 2);
    let event_types = events
        .iter()
        .map(|event| {
            event
                .try_get::<String>("", "event_type")
                .expect("event type")
        })
        .collect::<Vec<_>>();
    assert!(event_types.iter().any(|event| event == "request_held"));
    assert!(event_types.iter().any(|event| event == "request_resumed"));
}
