//! Integration tests for durable snapshot intents and post-purge data recovery.

use std::time::Duration;

use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataPostPurgeRecoveryService, ArtifactDataSnapshotIntentService, ModulesModule,
    PostPurgeRecoveryError, PrepareRecoveryRequest, SnapshotCopyKind,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

#[tokio::test]
async fn test_snapshot_intent_lifecycle_and_stale_orphan_reconciliation() {
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

    let intent_service = ArtifactDataSnapshotIntentService::new(database.clone());
    let tenant_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();

    // 1. Normal lifecycle: reserve -> staging -> committed
    let intent_id = intent_service
        .reserve_intent(
            tenant_id,
            snapshot_id,
            SnapshotCopyKind::Snapshot,
            "product-images/catalog.png",
            "source/key/1",
            "target/key/1",
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            1024,
        )
        .await
        .expect("reserve intent");

    intent_service
        .record_staging_receipt(intent_id)
        .await
        .expect("record staging receipt");

    intent_service
        .commit_intent(intent_id)
        .await
        .expect("commit intent");

    // 2. Reconciliation test: setup two stale intents created 10 minutes ago
    // Intent A: Parent snapshot ABORTED/FAILED (no ready row in snapshots table) -> should be collected as orphan
    let aborted_snapshot_id = Uuid::new_v4();
    let stale_intent_a = intent_service
        .reserve_intent(
            tenant_id,
            aborted_snapshot_id,
            SnapshotCopyKind::Snapshot,
            "orphan-file.png",
            "source/orphan",
            "target/orphan",
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            2048,
        )
        .await
        .expect("reserve stale intent a");
    intent_service
        .record_staging_receipt(stale_intent_a)
        .await
        .expect("staging intent a");

    // Intent B: Parent snapshot COMPLETED ('ready' in snapshots table) -> should be resumed and committed
    let completed_snapshot_id = Uuid::new_v4();
    let stale_intent_b = intent_service
        .reserve_intent(
            tenant_id,
            completed_snapshot_id,
            SnapshotCopyKind::Snapshot,
            "valid-file.png",
            "source/valid",
            "target/valid",
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            4096,
        )
        .await
        .expect("reserve stale intent b");
    intent_service
        .record_staging_receipt(stale_intent_b)
        .await
        .expect("staging intent b");

    // Insert 'ready' row for completed_snapshot_id
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
             ) VALUES (?1, ?2, 'media', 1, 1, 1, 'ready', 1, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000001', \
                'sha256:3333333333333333333333333333333333333333333333333333333333333333', \
                ?3, 'trace-1', ?4, 'test snapshot', ?5, 1, 1, 4096, ?6, 0, ?7, ?7)",
            vec![
                completed_snapshot_id.to_string().into(),
                tenant_id.to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                future_str.into(),
                now_str.into(),
            ],
        ))
        .await
        .expect("insert ready snapshot");

    // Make intents appear 10 minutes old in database
    let past_str = (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_data_snapshot_copy_intents SET created_at = ?1 \
             WHERE intent_id IN (?2, ?3)",
            vec![
                past_str.into(),
                stale_intent_a.to_string().into(),
                stale_intent_b.to_string().into(),
            ],
        ))
        .await
        .expect("backdate intents");

    // Reconcile with 5 minute grace period
    let receipt = intent_service
        .reconcile_stale_intents(tenant_id, Duration::from_secs(300))
        .await
        .expect("reconcile stale intents");

    assert_eq!(receipt.total_scanned, 2);
    assert_eq!(receipt.orphans_collected, 1);
    assert_eq!(receipt.committed_resumed, 1);

    // Verify intent A is collected
    let status_a: String = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT status FROM module_artifact_data_snapshot_copy_intents WHERE intent_id = ?1",
            vec![stale_intent_a.to_string().into()],
        ))
        .await
        .expect("query intent a")
        .expect("row")
        .try_get("", "status")
        .expect("get status");
    assert_eq!(status_a, "collected");

    // Verify intent B is committed
    let status_b: String = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT status FROM module_artifact_data_snapshot_copy_intents WHERE intent_id = ?1",
            vec![stale_intent_b.to_string().into()],
        ))
        .await
        .expect("query intent b")
        .expect("row")
        .try_get("", "status")
        .expect("get status");
    assert_eq!(status_b, "committed");
}

#[tokio::test]
async fn test_post_purge_recovery_staging_and_cas_cutover() {
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

    let recovery_service = ArtifactDataPostPurgeRecoveryService::new(database.clone());
    let tenant_id = Uuid::new_v4();
    let module_slug = "catalog";
    let data_contract_revision = 1u64;
    let tombstone_rev = 3u64;

    // 1. Create purged namespace with active purge tombstone
    let purged_at_str = Utc::now().to_rfc3339();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces (\
                tenant_id, module_slug, data_contract_revision, namespace_revision, \
                purged_at, created_at, updated_at\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)",
            vec![
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
                (tombstone_rev as i64).into(),
                purged_at_str.into(),
            ],
        ))
        .await
        .expect("insert purged namespace");

    // Also record historical purge operation
    let purge_idempotency = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_purge_operations (\
                tenant_id, module_slug, data_contract_revision, policy_revision, \
                idempotency_key, expected_namespace_revision, namespace_revision, \
                actor_id, trace_id, correlation_id, reason, purged_records, completed_at\
             ) VALUES (?1, ?2, ?3, 1, ?4, 2, ?5, ?6, 'trace-purge', ?7, 'tenant requested purge', 100, ?8)",
            vec![
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
                purge_idempotency.to_string().into(),
                (tombstone_rev as i64).into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                Utc::now().to_rfc3339().into(),
            ],
        ))
        .await
        .expect("insert purge operation");

    // 2. Create ready snapshot
    let snapshot_id = Uuid::new_v4();
    let future_str = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    let now_str = Utc::now().to_rfc3339();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_snapshots (\
                snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision, \
                source_namespace_revision, status, retention_revision, request_digest, \
                manifest_digest, actor_id, trace_id, correlation_id, reason, idempotency_key, \
                structured_record_count, object_count, total_object_bytes, retain_until, \
                legal_hold, created_at, ready_at\
             ) VALUES (?1, ?2, ?3, ?4, 1, 2, 'ready', 1, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000002', \
                'sha256:4444444444444444444444444444444444444444444444444444444444444444', \
                ?5, 'trace-snap', ?6, 'valid snapshot', ?7, 50, 4, 8192, ?8, 0, ?9, ?9)",
            vec![
                snapshot_id.to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                future_str.into(),
                now_str.into(),
            ],
        ))
        .await
        .expect("insert snapshot");

    // 3. Step A: Prepare recovery in isolated staging context
    let prep_req = PrepareRecoveryRequest {
        tenant_id,
        module_slug: module_slug.to_string(),
        data_contract_revision,
        source_snapshot_id: snapshot_id,
        actor_id: Uuid::new_v4(),
        trace_id: "trace-recovery-1".to_string(),
        correlation_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
    };

    let staged_receipt = recovery_service
        .prepare_recovery(prep_req)
        .await
        .expect("prepare recovery succeeds");

    assert_eq!(staged_receipt.status, "staging");
    assert_eq!(staged_receipt.tombstone_namespace_revision, 3);
    assert_eq!(staged_receipt.target_namespace_revision, 4);
    assert_eq!(staged_receipt.records_restored, 50);
    assert_eq!(staged_receipt.objects_restored, 4);

    // 4. Step B: Premature cutover before verification must fail
    let premature_cutover_err = recovery_service
        .execute_cas_cutover(staged_receipt.recovery_id)
        .await
        .expect_err("premature cutover must fail");
    assert!(matches!(
        premature_cutover_err,
        PostPurgeRecoveryError::InvalidRecoveryState { .. }
    ));

    // 5. Step C: Verify staged recovery
    let verified_receipt = recovery_service
        .verify_staged_recovery(staged_receipt.recovery_id)
        .await
        .expect("verify staged recovery");
    assert_eq!(verified_receipt.status, "verified");

    // 6. Step D: Execute authorized CAS cutover
    let cutover_receipt = recovery_service
        .execute_cas_cutover(staged_receipt.recovery_id)
        .await
        .expect("execute CAS cutover");

    assert_eq!(cutover_receipt.active_namespace_revision, 4);
    assert_eq!(cutover_receipt.records_restored, 50);
    assert_eq!(cutover_receipt.objects_restored, 4);

    // 7. Verify active namespace state: active at revision 4, purged_at IS NULL
    let active_row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT namespace_revision, purged_at FROM module_artifact_data_namespaces \
             WHERE tenant_id = ?1 AND module_slug = ?2 AND data_contract_revision = ?3",
            vec![
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
            ],
        ))
        .await
        .expect("query active namespace")
        .expect("row");

    let active_rev: i64 = active_row.try_get("", "namespace_revision").expect("rev");
    let active_purged: Option<String> = active_row.try_get("", "purged_at").expect("purged_at");
    assert_eq!(active_rev, 4);
    assert_eq!(active_purged, None, "active namespace must not be purged");

    // 8. ARCHITECTURAL INVARIANT: "never clear the old purge tombstone"
    // Verify that the historical purge operations table still contains the immutable tombstone
    let purge_op_count: i64 = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS c FROM module_artifact_data_purge_operations \
             WHERE tenant_id = ?1 AND module_slug = ?2 AND data_contract_revision = ?3",
            vec![
                tenant_id.to_string().into(),
                module_slug.into(),
                (data_contract_revision as i64).into(),
            ],
        ))
        .await
        .expect("query purge ops")
        .expect("row")
        .try_get("", "c")
        .expect("count");
    assert_eq!(purge_op_count, 1, "purge tombstone history must remain intact");

    // 9. Negative test: Attempt to prepare recovery for non-purged namespace fails
    let bad_prep = PrepareRecoveryRequest {
        tenant_id,
        module_slug: module_slug.to_string(),
        data_contract_revision,
        source_snapshot_id: snapshot_id,
        actor_id: Uuid::new_v4(),
        trace_id: "trace-bad".to_string(),
        correlation_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
    };
    let err = recovery_service
        .prepare_recovery(bad_prep)
        .await
        .expect_err("must reject recovery for non-purged namespace");
    assert!(matches!(err, PostPurgeRecoveryError::NamespaceNotPurged { .. }));
}
