//! Integration tests for separately signed operations-tool releases, protocol matrix,
//! fleet-level exclusion fences, supervisor reports, and single-attempt predecessor recovery authorization.

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{Duration, Utc};
use ed25519_dalek::{Signer, SigningKey};
use rustok_core::MigrationSource;
use rustok_modules::{
    CURRENT_OPERATIONS_TOOL_PROTOCOL, ConflictFenceSet, ModulesModule,
    OPERATIONS_TOOL_RELEASE_CONTRACT, OperationsToolComponent, OperationsToolError,
    OperationsToolProtocolMatrix, OperationsToolRelease, OperationsToolReleasePayload,
    OperationsToolService, OperationsToolSupervisorReport,
    StartOperationsToolMaintenanceCommand,
};
use sea_orm::Database;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite connects");
    let schema_manager = SchemaManager::new(&db);
    rustok_outbox::SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("outbox migration succeeds");
    for migration in ModulesModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("migration succeeds");
    }
    db
}

fn create_keypair(seed_byte: u8) -> (SigningKey, String, String) {
    let seed = [seed_byte; 32];
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    let public_key_bytes = verifying_key.to_bytes();
    let public_key_base64 = STANDARD.encode(public_key_bytes);
    let public_key_digest = sha256_digest(&public_key_bytes);
    (signing_key, public_key_base64, public_key_digest)
}

fn create_signed_release(
    signing_key: &SigningKey,
    signer_key_digest: &str,
    release_id: Uuid,
    version: &str,
    protocol_revision: u32,
    package_name: &str,
) -> OperationsToolRelease {
    let now = Utc::now();
    let payload = OperationsToolReleasePayload {
        contract: OPERATIONS_TOOL_RELEASE_CONTRACT.to_string(),
        release_id,
        version: version.to_string(),
        protocol_revision,
        package_digest: sha256_digest(format!("pkg-{}", package_name).as_bytes()),
        controller_digest: sha256_digest(format!("ctrl-{}", package_name).as_bytes()),
        reconciler_digest: sha256_digest(format!("rec-{}", package_name).as_bytes()),
        agent_digest: sha256_digest(format!("agent-{}", package_name).as_bytes()),
        signer_key_digest: signer_key_digest.to_string(),
        issued_at: now - Duration::minutes(5),
        expires_at: now + Duration::hours(24),
    };

    let canonical_bytes = rustok_api::manifest_hash::canonical_json_bytes(&payload)
        .expect("canonical json serialization succeeds");
    let signature = signing_key.sign(&canonical_bytes);
    let signature_base64 = STANDARD.encode(signature.to_bytes());

    OperationsToolRelease {
        payload,
        signature: signature_base64,
    }
}

#[tokio::test]
async fn test_signed_release_publication_and_signature_verification() {
    let db = setup_test_db().await;
    let (signing_key, pubkey_b64, pubkey_digest) = create_keypair(1);
    let service = OperationsToolService::new(db, pubkey_b64.clone());

    let release_id = Uuid::new_v4();
    let release = create_signed_release(
        &signing_key,
        &pubkey_digest,
        release_id,
        "1.0.0",
        CURRENT_OPERATIONS_TOOL_PROTOCOL,
        "release-v1",
    );

    let now = Utc::now();
    let verified = service
        .publish_release(release.clone(), now)
        .await
        .expect("valid release publication succeeds");

    assert_eq!(verified.payload().release_id, release_id);
    assert_eq!(verified.payload().version, "1.0.0");

    // 1. Verify tamper detection: corrupted signature is rejected
    let mut tampered = release.clone();
    tampered.signature = STANDARD.encode([0u8; 64]);
    let err = service.publish_release(tampered, now).await.unwrap_err();
    assert_eq!(err, OperationsToolError::SignatureRejected);

    // 2. Expired release is rejected
    let mut expired = release.clone();
    expired.payload.issued_at = now - Duration::hours(10);
    expired.payload.expires_at = now - Duration::hours(1);
    let expired_bytes = rustok_api::manifest_hash::canonical_json_bytes(&expired.payload).unwrap();
    expired.signature = STANDARD.encode(signing_key.sign(&expired_bytes).to_bytes());
    let err = service.publish_release(expired, now).await.unwrap_err();
    assert_eq!(err, OperationsToolError::Expired);

    // 3. Mismatched signer key digest is rejected
    let mut bad_signer = release;
    bad_signer.payload.signer_key_digest = sha256_digest(b"unrelated-key");
    let bad_bytes = rustok_api::manifest_hash::canonical_json_bytes(&bad_signer.payload).unwrap();
    bad_signer.signature = STANDARD.encode(signing_key.sign(&bad_bytes).to_bytes());
    let err = service.publish_release(bad_signer, now).await.unwrap_err();
    assert_eq!(err, OperationsToolError::InvalidPublicKey);
}

#[tokio::test]
async fn test_protocol_matrix_and_preflight_checks() {
    let db = setup_test_db().await;
    let (signing_key, pubkey_b64, pubkey_digest) = create_keypair(2);
    let service = OperationsToolService::new(db, pubkey_b64);

    let now = Utc::now();

    // Release with incompatible protocol revision (e.g. 99)
    let incompatible_id = Uuid::new_v4();
    let incompatible_release = create_signed_release(
        &signing_key,
        &pubkey_digest,
        incompatible_id,
        "2.0.0",
        99,
        "incompatible-v2",
    );
    service
        .publish_release(incompatible_release, now)
        .await
        .expect("publish succeeds");

    let preflight_err = service
        .verify_preflight(incompatible_id, CURRENT_OPERATIONS_TOOL_PROTOCOL, now)
        .await
        .unwrap_err();

    match preflight_err {
        OperationsToolError::ProtocolIncompatible {
            owner_protocol,
            tool_protocol,
        } => {
            assert_eq!(owner_protocol, CURRENT_OPERATIONS_TOOL_PROTOCOL);
            assert_eq!(tool_protocol, 99);
        }
        other => panic!("expected ProtocolIncompatible, got {other:?}"),
    }

    // Missing release returns NotFound
    let missing_id = Uuid::new_v4();
    let missing_err = service
        .verify_preflight(missing_id, CURRENT_OPERATIONS_TOOL_PROTOCOL, now)
        .await
        .unwrap_err();
    assert_eq!(missing_err, OperationsToolError::NotFound(missing_id));

    // Matrix custom compatibility
    let matrix = OperationsToolProtocolMatrix {
        supported_protocols: vec![1, 2],
    };
    assert!(matrix.is_compatible(1, 1));
    assert!(matrix.is_compatible(2, 2));
    assert!(!matrix.is_compatible(1, 2));
    assert!(!matrix.is_compatible(3, 3));
}

#[tokio::test]
async fn test_fleet_exclusion_fence_and_maintenance_start() {
    let db = setup_test_db().await;
    let (signing_key, pubkey_b64, pubkey_digest) = create_keypair(3);
    let service = OperationsToolService::new(db, pubkey_b64);
    let now = Utc::now();

    // Verify ConflictFenceSet semantics for fleet_operations_tool
    let tool_fences = ConflictFenceSet::derive_operations_tool_maintenance_fences();
    let conflicting_fences = ConflictFenceSet::derive_operations_tool_maintenance_fences();
    assert!(tool_fences.has_conflict_with(&conflicting_fences));

    let unrelated_fences = ConflictFenceSet::derive_module_update_fences(
        "unrelated-mod",
        None,
        &["node-gamma".to_string()],
    );
    assert!(!tool_fences.has_conflict_with(&unrelated_fences));

    // Publish candidate release
    let target_id = Uuid::new_v4();
    let target_release = create_signed_release(
        &signing_key,
        &pubkey_digest,
        target_id,
        "1.1.0",
        CURRENT_OPERATIONS_TOOL_PROTOCOL,
        "fleet-v1.1",
    );
    service.publish_release(target_release, now).await.unwrap();

    let op_id = Uuid::new_v4();
    let command = StartOperationsToolMaintenanceCommand {
        operation_id: op_id,
        target_release_id: target_id,
        predecessor_release_id: None,
        host_ids: vec!["host-alpha".to_string(), "host-beta".to_string()],
        actor_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-op-1".to_string(),
        correlation_id: Uuid::new_v4(),
    };

    let op = service
        .start_maintenance(command, now)
        .await
        .expect("start maintenance succeeds");

    assert_eq!(op.operation_id, op_id);
    assert_eq!(op.status, "in_progress");
    assert_eq!(op.recovery_attempts, 0);

    // Verify 6 assignments (2 hosts * 3 components) created in staged status
    let alpha_ctrl = service
        .get_assignment(op_id, "host-alpha", OperationsToolComponent::Controller)
        .await
        .unwrap();
    assert_eq!(alpha_ctrl.status, "staged");
    assert!(alpha_ctrl.observed_digest.is_none());

    let beta_agent = service
        .get_assignment(op_id, "host-beta", OperationsToolComponent::Agent)
        .await
        .unwrap();
    assert_eq!(beta_agent.status, "staged");
    assert!(beta_agent.observed_digest.is_none());
}

#[tokio::test]
async fn test_supervisor_reports_and_automatic_convergence() {
    let db = setup_test_db().await;
    let (signing_key, pubkey_b64, pubkey_digest) = create_keypair(4);
    let service = OperationsToolService::new(db, pubkey_b64);
    let now = Utc::now();

    let target_id = Uuid::new_v4();
    let target_release = create_signed_release(
        &signing_key,
        &pubkey_digest,
        target_id,
        "1.2.0",
        CURRENT_OPERATIONS_TOOL_PROTOCOL,
        "target-v1.2",
    );
    let verified_target = service.publish_release(target_release, now).await.unwrap();

    let op_id = Uuid::new_v4();
    let hosts = vec!["host-1".to_string(), "host-2".to_string()];
    let command = StartOperationsToolMaintenanceCommand {
        operation_id: op_id,
        target_release_id: target_id,
        predecessor_release_id: None,
        host_ids: hosts.clone(),
        actor_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-op-2".to_string(),
        correlation_id: Uuid::new_v4(),
    };
    service.start_maintenance(command, now).await.unwrap();

    let components = [
        (
            OperationsToolComponent::Controller,
            &verified_target.payload().controller_digest,
        ),
        (
            OperationsToolComponent::Reconciler,
            &verified_target.payload().reconciler_digest,
        ),
        (
            OperationsToolComponent::Agent,
            &verified_target.payload().agent_digest,
        ),
    ];

    // Report observations for all components across hosts except the last one
    for host in &hosts {
        for (comp, digest) in &components {
            if host == "host-2" && *comp == OperationsToolComponent::Agent {
                continue; // leave one pending
            }
            service
                .report_supervisor_observation(
                    OperationsToolSupervisorReport {
                        operation_id: op_id,
                        host_id: host.clone(),
                        component: *comp,
                        observed_digest: (*digest).clone(),
                        status: "converged".to_string(),
                    },
                    now,
                )
                .await
                .unwrap();
        }
    }

    // Operation must still be in_progress
    let op_pending = service.get_operation(op_id).await.unwrap();
    assert_eq!(op_pending.status, "in_progress");

    // Report final component
    service
        .report_supervisor_observation(
            OperationsToolSupervisorReport {
                operation_id: op_id,
                host_id: "host-2".to_string(),
                component: OperationsToolComponent::Agent,
                observed_digest: verified_target.payload().agent_digest.clone(),
                status: "converged".to_string(),
            },
            now,
        )
        .await
        .unwrap();

    // All assignments converged -> operation converges automatically
    let op_converged = service.get_operation(op_id).await.unwrap();
    assert_eq!(op_converged.status, "converged");
}

#[tokio::test]
async fn test_predecessor_recovery_authorization_and_single_attempt_exhaustion() {
    let db = setup_test_db().await;
    let (signing_key, pubkey_b64, pubkey_digest) = create_keypair(5);
    let service = OperationsToolService::new(db, pubkey_b64);
    let now = Utc::now();

    // 1. Publish predecessor release
    let pred_id = Uuid::new_v4();
    let pred_release = create_signed_release(
        &signing_key,
        &pubkey_digest,
        pred_id,
        "1.0.0",
        CURRENT_OPERATIONS_TOOL_PROTOCOL,
        "pred-v1.0",
    );
    let verified_pred = service.publish_release(pred_release, now).await.unwrap();

    // 2. Publish target release
    let target_id = Uuid::new_v4();
    let target_release = create_signed_release(
        &signing_key,
        &pubkey_digest,
        target_id,
        "1.1.0",
        CURRENT_OPERATIONS_TOOL_PROTOCOL,
        "target-v1.1",
    );
    service.publish_release(target_release, now).await.unwrap();

    // 3. Start maintenance with predecessor configured
    let op_id = Uuid::new_v4();
    let command = StartOperationsToolMaintenanceCommand {
        operation_id: op_id,
        target_release_id: target_id,
        predecessor_release_id: Some(pred_id),
        host_ids: vec!["node-1".to_string()],
        actor_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-rec-1".to_string(),
        correlation_id: Uuid::new_v4(),
    };
    service.start_maintenance(command, now).await.unwrap();

    // 4. Authorize predecessor recovery (Attempt 1)
    let recovered_op = service
        .authorize_predecessor_recovery(op_id, now)
        .await
        .expect("predecessor recovery attempt 1 succeeds");

    assert_eq!(recovered_op.recovery_attempts, 1);
    assert_eq!(recovered_op.status, "rolled_back");

    // Desired digests on node-1 should now point to predecessor digests
    let node1_ctrl = service
        .get_assignment(op_id, "node-1", OperationsToolComponent::Controller)
        .await
        .unwrap();
    assert_eq!(
        node1_ctrl.desired_digest,
        verified_pred.payload().controller_digest
    );
    assert_eq!(node1_ctrl.status, "staged");

    let node1_rec = service
        .get_assignment(op_id, "node-1", OperationsToolComponent::Reconciler)
        .await
        .unwrap();
    assert_eq!(
        node1_rec.desired_digest,
        verified_pred.payload().reconciler_digest
    );
    assert_eq!(node1_rec.status, "staged");

    // 5. Attempting second recovery must fail with RecoveryExhausted (max 1 attempt)
    let second_err = service
        .authorize_predecessor_recovery(op_id, now)
        .await
        .unwrap_err();
    assert_eq!(second_err, OperationsToolError::RecoveryExhausted(op_id));

    // 6. Operation without predecessor cannot recover
    let no_pred_op_id = Uuid::new_v4();
    let no_pred_command = StartOperationsToolMaintenanceCommand {
        operation_id: no_pred_op_id,
        target_release_id: target_id,
        predecessor_release_id: None,
        host_ids: vec!["node-1".to_string()],
        actor_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-no-pred".to_string(),
        correlation_id: Uuid::new_v4(),
    };
    service
        .start_maintenance(no_pred_command, now)
        .await
        .unwrap();

    let err_no_pred = service
        .authorize_predecessor_recovery(no_pred_op_id, now)
        .await
        .unwrap_err();
    assert_eq!(
        err_no_pred,
        OperationsToolError::NoPredecessor(no_pred_op_id)
    );
}
