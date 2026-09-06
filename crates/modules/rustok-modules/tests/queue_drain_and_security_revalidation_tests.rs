//! Integration tests for security/quarantine revalidation before claim and bounded queue drain.

use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactEventDeliveryConfig, ArtifactQueueDrainRequest, ArtifactQueueDrainService,
    ModuleCommandContext, ModulesModule, SeaOrmArtifactEventDeliveryQueue,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

fn command_context(tenant_id: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: Some(tenant_id),
        idempotency_key: Uuid::new_v4(),
        trace_id: "test:queue-drain".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

#[tokio::test]
async fn test_event_claim_revalidates_security_state_and_dead_letters() {
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

    let tenant_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let delivery_id = Uuid::new_v4();
    let source_event_id = Uuid::new_v4();
    let module_slug = "analytics";
    let module_version = "1.0.0";

    // 1. Insert installation
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
                slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
                data_owner_id, settings_instance_id, dependency_graph_revision, dependency_graph_digest, \
                dependency_lock, installed_at\
             ) VALUES (?1, 'tenant', ?2, 'official', 'analytics', 'sha256:0000000000000000000000000000000000000000000000000000000000000000', \
                ?3, ?4, 'wasm', 'wasm:v1', 'sha256:0000000000000000000000000000000000000000000000000000000000000000', \
                'entrypoint.wasm', '{\"schema_version\":1,\"module\":{\"slug\":\"analytics\",\"version\":\"1.0.0\"}}', \
                ?5, ?6, 1, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', '[]', '2026-09-01T00:00:00Z')",
            vec![
                installation_id.to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                module_version.into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("insert installation");

    // 2. Insert active admission
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_admissions (\
                stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at\
             ) VALUES (?1, ?2, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', 'application/wasm', 1024, '{}', 'active', 1, '2026-09-01T00:00:00Z')",
            vec![Uuid::new_v4().to_string().into(), installation_id.to_string().into()],
        ))
        .await
        .expect("insert admission");

    // 3. Insert pending event delivery
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_event_deliveries (\
                delivery_id, tenant_id, source_event_id, installation_id, binding_id, event_type, \
                schema_version, payload, source_digest, attempt, status, available_at, created_at\
             ) VALUES (?1, ?2, ?3, ?4, 'evt_sub', 'order.completed', 1, '{}', \
                'sha256:0000000000000000000000000000000000000000000000000000000000000000', 0, 'pending', '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![
                delivery_id.to_string().into(),
                tenant_id.to_string().into(),
                source_event_id.to_string().into(),
                installation_id.to_string().into(),
            ],
        ))
        .await
        .expect("insert event delivery");

    // 4. Mark the module quarantined in module_artifact_security_states
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_security_states (\
                module_slug, module_version, payload_digest, revision, status, policy_revision, reason_code, reason_detail, changed_by, changed_at\
             ) VALUES (?1, ?2, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', 1, 'quarantined', 'policy-v1', 'security_vulnerability', 'critical vulnerability', ?3, '2026-09-02T00:00:00Z')",
            vec![
                module_slug.into(),
                module_version.into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("insert quarantined security state");

    // 5. Worker tries to claim next item
    let queue = SeaOrmArtifactEventDeliveryQueue::new(
        database.clone(),
        ArtifactEventDeliveryConfig::default(),
    )
    .expect("queue construction succeeds");
    let claimed = queue
        .claim_next(tenant_id, "worker-1")
        .await
        .expect("claim_next succeeds");
    assert!(claimed.is_none(), "quarantined item must not be claimed");

    // 6. Verify delivery was dead-lettered with revoked_or_quarantined error code
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT status, last_error_code, dead_lettered_at FROM module_artifact_event_deliveries WHERE delivery_id = ?1",
            vec![delivery_id.to_string().into()],
        ))
        .await
        .expect("query delivery")
        .unwrap();
    let status: String = row.try_get("", "status").unwrap();
    let last_error: String = row.try_get("", "last_error_code").unwrap();
    assert_eq!(status, "dead_letter");
    assert_eq!(last_error, "revoked_or_quarantined");
}

#[tokio::test]
async fn test_bounded_queue_drain_predecessor_incompatible() {
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

    let tenant_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();

    // 1. Insert installation
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
                slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
                data_owner_id, settings_instance_id, dependency_graph_revision, dependency_graph_digest, \
                dependency_lock, installed_at\
             ) VALUES (?1, 'tenant', ?2, 'official', 'billing', 'sha256:0000000000000000000000000000000000000000000000000000000000000000', \
                'billing', '1.0.0', 'wasm', 'wasm:v1', 'sha256:0000000000000000000000000000000000000000000000000000000000000000', \
                'entrypoint.wasm', '{\"schema_version\":1,\"module\":{\"slug\":\"billing\",\"version\":\"1.0.0\"}}', \
                ?3, ?4, 1, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', '[]', '2026-09-01T00:00:00Z')",
            vec![
                installation_id.to_string().into(),
                tenant_id.to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("insert installation");

    // 2. Insert 3 event deliveries and 3 schedule deliveries
    for i in 1..=3 {
        let evt_id = Uuid::new_v4();
        let src_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_event_deliveries (\
                    delivery_id, tenant_id, source_event_id, installation_id, binding_id, event_type, \
                    schema_version, payload, source_digest, attempt, status, available_at, created_at\
                 ) VALUES (?1, ?2, ?3, ?4, 'evt_sub', 'invoice.created', 1, '{}', \
                    'sha256:0000000000000000000000000000000000000000000000000000000000000000', 0, 'pending', '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
                vec![
                    evt_id.to_string().into(),
                    tenant_id.to_string().into(),
                    src_id.to_string().into(),
                    installation_id.to_string().into(),
                ],
            ))
            .await
            .expect("insert event delivery");

        let sched_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_schedule_deliveries (\
                    delivery_id, tenant_id, installation_id, binding_id, schedule_digest, scheduled_for, \
                    attempt, status, available_at, created_at\
                 ) VALUES (?1, ?2, ?3, ?4, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', \
                    '2026-09-01T00:00:00Z', 0, 'pending', '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
                vec![
                    sched_id.to_string().into(),
                    tenant_id.to_string().into(),
                    installation_id.to_string().into(),
                    format!("binding_{}", i).into(),
                ],
            ))
            .await
            .expect("insert schedule delivery");
    }

    let drain_service = ArtifactQueueDrainService::new(database.clone());

    // 3. Drain first batch (limit = 2)
    let batch1_req = ArtifactQueueDrainRequest {
        tenant_id,
        installation_id,
        limit: 2,
        context: command_context(tenant_id),
        reason: "predecessor-incompatible drain batch 1".to_string(),
    };
    let batch1_res = drain_service
        .drain_incompatible_work(batch1_req)
        .await
        .expect("batch 1 drain succeeds");
    assert_eq!(batch1_res.drained_events, 2);
    assert_eq!(batch1_res.drained_schedules, 2);
    assert_eq!(batch1_res.remaining_pending, 2); // 1 event + 1 schedule left

    // 4. Drain second batch (limit = 10)
    let batch2_req = ArtifactQueueDrainRequest {
        tenant_id,
        installation_id,
        limit: 10,
        context: command_context(tenant_id),
        reason: "predecessor-incompatible drain batch 2".to_string(),
    };
    let batch2_res = drain_service
        .drain_incompatible_work(batch2_req)
        .await
        .expect("batch 2 drain succeeds");
    assert_eq!(batch2_res.drained_events, 1);
    assert_eq!(batch2_res.drained_schedules, 1);
    assert_eq!(batch2_res.remaining_pending, 0); // All cleared
}
