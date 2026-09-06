//! Integration tests for bounded artifact-data snapshot readiness and platform recovery evidence.

use std::time::Duration;

use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataRecoveryReadinessService, ModulesModule,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

#[tokio::test]
async fn test_snapshot_readiness_and_attestation_lifecycle() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("module migration");
    }

    let service = ArtifactDataRecoveryReadinessService::new(database.clone());
    let tenant_id = Uuid::new_v4();
    let module_slug = "orders";
    let data_contract_revision = 1u64;

    // 1. Initially: No snapshot exists -> not ready
    let initial_readiness = service
        .evaluate_snapshot_readiness(
            tenant_id,
            module_slug,
            data_contract_revision,
            Duration::from_secs(3600),
        )
        .await
        .expect("evaluate initial readiness");
    assert!(!initial_readiness.ready);
    assert!(!initial_readiness.is_within_sla);
    assert_eq!(initial_readiness.snapshot_id, None);

    // 2. Snapshot exists but is in 'staging' status -> not ready
    let snapshot_id_staging = Uuid::new_v4();
    let idempotency_key_staging = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_snapshots (\
                snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision, \
                source_namespace_revision, status, retention_revision, request_digest, \
                manifest_digest, actor_id, trace_id, correlation_id, reason, idempotency_key, \
                structured_record_count, object_count, total_object_bytes, retain_until, \
                legal_hold, created_at, ready_at\
             ) VALUES (?1, ?2, ?3, ?4, 1, 1, 'staging', 1, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000001', \
                NULL, ?5, 'trace-1', ?6, 'pre-deploy snapshot', ?7, 0, 0, 0, \
                '2030-01-01T00:00:00Z', 0, '2026-09-03T20:00:00Z', NULL)",
            vec![
                snapshot_id_staging.to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                idempotency_key_staging.to_string().into(),
            ],
        ))
        .await
        .expect("insert staging snapshot");

    let staging_readiness = service
        .evaluate_snapshot_readiness(
            tenant_id,
            module_slug,
            data_contract_revision,
            Duration::from_secs(3600),
        )
        .await
        .expect("evaluate staging readiness");
    assert!(!staging_readiness.ready, "staging snapshot cannot be considered ready");

    // 3. Snapshot is 'ready', valid manifest, created just now -> fully ready and within SLA
    let snapshot_id_ready = Uuid::new_v4();
    let idempotency_key_ready = Uuid::new_v4();
    let now_str = Utc::now().to_rfc3339();
    let future_str = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_snapshots (\
                snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision, \
                source_namespace_revision, status, retention_revision, request_digest, \
                manifest_digest, actor_id, trace_id, correlation_id, reason, idempotency_key, \
                structured_record_count, object_count, total_object_bytes, retain_until, \
                legal_hold, created_at, ready_at\
             ) VALUES (?1, ?2, ?3, ?4, 1, 1, 'ready', 1, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000002', \
                'sha256:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd', \
                ?5, 'trace-2', ?6, 'pre-deploy ready snapshot', ?7, 42, 5, 2048, \
                ?8, 0, ?9, ?9)",
            vec![
                snapshot_id_ready.to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                idempotency_key_ready.to_string().into(),
                future_str.clone().into(),
                now_str.into(),
            ],
        ))
        .await
        .expect("insert ready snapshot");

    let ready_readiness = service
        .evaluate_snapshot_readiness(
            tenant_id,
            module_slug,
            data_contract_revision,
            Duration::from_secs(3600),
        )
        .await
        .expect("evaluate ready snapshot");
    assert!(ready_readiness.ready);
    assert!(ready_readiness.is_within_sla);
    assert_eq!(ready_readiness.snapshot_id, Some(snapshot_id_ready));
    assert_eq!(ready_readiness.structured_record_count, Some(42));
    assert_eq!(ready_readiness.object_count, Some(5));

    // 4. Stale snapshot check: snapshot created 48 hours ago evaluated with a 1 hour SLA
    let snapshot_id_stale = Uuid::new_v4();
    let idempotency_key_stale = Uuid::new_v4();
    let old_created_at = (Utc::now() - chrono::Duration::hours(48)).to_rfc3339();

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_snapshots (\
                snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision, \
                source_namespace_revision, status, retention_revision, request_digest, \
                manifest_digest, actor_id, trace_id, correlation_id, reason, idempotency_key, \
                structured_record_count, object_count, total_object_bytes, retain_until, \
                legal_hold, created_at, ready_at\
             ) VALUES (?1, ?2, ?3, 2, 1, 1, 'ready', 1, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000004', \
                'sha256:4444444444444444444444444444444444444444444444444444444444444444', \
                ?4, 'trace-4', ?5, 'old snapshot', ?6, 10, 1, 512, \
                ?7, 0, ?8, ?8)",
            vec![
                snapshot_id_stale.to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                idempotency_key_stale.to_string().into(),
                future_str.into(),
                old_created_at.into(),
            ],
        ))
        .await
        .expect("insert stale snapshot");

    let stale_readiness = service
        .evaluate_snapshot_readiness(
            tenant_id,
            module_slug,
            2u64,
            Duration::from_secs(3600), // SLA is 1 hour, but snapshot was created 48 hours ago
        )
        .await
        .expect("evaluate stale snapshot");
    assert!(!stale_readiness.is_within_sla);
    assert!(!stale_readiness.ready, "snapshot outside SLA cannot be ready");
}

#[tokio::test]
async fn test_platform_recovery_evidence_and_no_automatic_restore_invariant() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("module migration");
    }

    let service = ArtifactDataRecoveryReadinessService::new(database.clone());
    let tenant_id = Uuid::new_v4();
    let module_slug = "customer";
    let data_contract_revision = 1u64;

    // Insert a valid snapshot
    let snapshot_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();
    let now_str = Utc::now().to_rfc3339();
    let future_str = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_snapshots (\
                snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision, \
                source_namespace_revision, status, retention_revision, request_digest, \
                manifest_digest, actor_id, trace_id, correlation_id, reason, idempotency_key, \
                structured_record_count, object_count, total_object_bytes, retain_until, \
                legal_hold, created_at, ready_at\
             ) VALUES (?1, ?2, ?3, ?4, 1, 1, 'ready', 1, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000003', \
                'sha256:11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff', \
                ?5, 'trace-3', ?6, 'validated snapshot', ?7, 100, 2, 1024, \
                ?8, 0, ?9, ?9)",
            vec![
                snapshot_id.to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                idempotency_key.to_string().into(),
                future_str.into(),
                now_str.into(),
            ],
        ))
        .await
        .expect("insert snapshot");

    // 1. Evaluate platform recovery evidence
    let platform_evidence = service
        .evaluate_platform_recovery_evidence()
        .await
        .expect("platform evidence");
    assert!(platform_evidence.recovery_capable);
    assert!(platform_evidence.evidence_digest.starts_with("sha256:"));
    assert!(platform_evidence.checkpoint_lsn_or_tag.contains("page_count="));

    // 2. Attest complete recovery readiness
    let attestation = service
        .attest_recovery_readiness(
            tenant_id,
            module_slug,
            data_contract_revision,
            Duration::from_secs(86400),
        )
        .await
        .expect("attest recovery readiness");

    assert_eq!(attestation.tenant_id, tenant_id);
    assert_eq!(attestation.module_slug, module_slug);
    assert!(attestation.snapshot.ready);
    assert!(attestation.platform_evidence.recovery_capable);

    // 3. ARCHITECTURAL INVARIANT: Automatic restore is strictly and unconditionally FORBIDDEN.
    // Proves line 2018-2019: "without adding automatic restore".
    assert!(
        !attestation.automatic_restore_authorized,
        "Platform architecture strictly forbids automatic restore; data recovery requires explicit operator authorization"
    );
}
