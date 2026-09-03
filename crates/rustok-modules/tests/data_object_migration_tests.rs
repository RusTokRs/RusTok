//! Integration tests for broker-owned object migration service and live-object guard.

use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataCopyError, ArtifactDataCrossRevisionCopier, ArtifactDataObjectMigrationError,
    ArtifactDataObjectMigrationRequest, ArtifactDataObjectMigrationService, ModuleCommandContext,
    ModulesModule,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

fn command_context(tenant_id: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: Some(tenant_id),
        idempotency_key: Uuid::new_v4(),
        trace_id: "test:object-migration".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

#[tokio::test]
async fn test_object_migration_lifecycle_acceptance_and_live_object_guard() {
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
    let module_slug = "media";

    // 1. Create namespaces
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at) VALUES (?1, ?2, 1, 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![tenant_id.to_string().into(), module_slug.into()],
        ))
        .await
        .expect("insert source namespace");

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at) VALUES (?1, ?2, 2, 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![tenant_id.to_string().into(), module_slug.into()],
        ))
        .await
        .expect("insert target namespace");

    // 2. Insert 3 source objects in revision 1
    let objects = [
        ("avatar.png", "image/png", 2048, "sha256:1111111111111111111111111111111111111111111111111111111111111111", "storage/avatar"),
        ("document.pdf", "application/pdf", 1048576, "sha256:2222222222222222222222222222222222222222222222222222222222222222", "storage/doc"),
        ("photo.jpg", "image/jpeg", 524288, "sha256:3333333333333333333333333333333333333333333333333333333333333333", "storage/photo"),
    ];

    for (name, content_type, size, digest, key) in objects {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_data_objects (\
                    tenant_id, module_slug, data_contract_revision, object_name, storage_key, \
                    content_type, size_bytes, digest_sha256, revision, created_at, updated_at\
                 ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?7, 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
                vec![
                    tenant_id.to_string().into(),
                    module_slug.into(),
                    name.into(),
                    key.into(),
                    content_type.into(),
                    size.into(),
                    digest.into(),
                ],
            ))
            .await
            .expect("insert object");
    }

    let obj_service = ArtifactDataObjectMigrationService::new(database.clone());
    let data_copier = ArtifactDataCrossRevisionCopier::new(database.clone());

    // 3. Live objects guard: ensure structured copy / revision change is DENIED
    // before object migration has run
    let unmigrated = obj_service
        .count_unmigrated_live_objects(tenant_id, module_slug, 1, 2)
        .await
        .expect("count unmigrated objects");
    assert_eq!(unmigrated, 3, "all 3 objects must be unmigrated");

    let guard_err = data_copier
        .ensure_no_unmigrated_live_objects(tenant_id, module_slug, 1, 2)
        .await
        .expect_err("must deny when live objects are unmigrated");
    assert_eq!(guard_err, ArtifactDataCopyError::UnmigratedLiveObjects(3));

    // 4. Perform broker-owned object migration
    let req = ArtifactDataObjectMigrationRequest {
        tenant_id,
        module_slug: module_slug.to_string(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        context: command_context(tenant_id),
        reason: "maintenance object migration for revision 2".to_string(),
    };

    let receipt = obj_service
        .migrate_objects(req)
        .await
        .expect("migration succeeds");
    assert_eq!(receipt.objects_migrated, 3);
    assert_eq!(receipt.accepted, true);
    assert!(receipt.inventory_manifest_digest.starts_with("sha256:"));

    // 5. Verify target objects and guard passing
    let remaining_unmigrated = obj_service
        .count_unmigrated_live_objects(tenant_id, module_slug, 1, 2)
        .await
        .expect("count unmigrated objects after copy");
    assert_eq!(remaining_unmigrated, 0, "no unmigrated objects should remain");

    data_copier
        .ensure_no_unmigrated_live_objects(tenant_id, module_slug, 1, 2)
        .await
        .expect("guard must pass now that all objects are migrated");

    // 6. Verify checkpoint operations exist and are marked 'checkpointed'
    let ops_row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM module_artifact_data_object_copy_operations WHERE tenant_id = ?1 AND status = 'checkpointed'",
            vec![tenant_id.to_string().into()],
        ))
        .await
        .expect("query ops")
        .unwrap();
    let ops_count: i64 = ops_row.try_get("", "count").unwrap();
    assert_eq!(ops_count, 3);
}

#[tokio::test]
async fn test_object_migration_conflict_detection_and_reconciliation() {
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
    let module_slug = "uploads";

    // 1. Insert source object in revision 1
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_objects (\
                tenant_id, module_slug, data_contract_revision, object_name, storage_key, \
                content_type, size_bytes, digest_sha256, revision, created_at, updated_at\
             ) VALUES (?1, ?2, 1, 'banner.png', 'k1', 'image/png', 100, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![tenant_id.to_string().into(), module_slug.into()],
        ))
        .await
        .expect("insert source object");

    // 2. Pre-insert conflicting object in target revision 2 with DIFFERENT digest
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_objects (\
                tenant_id, module_slug, data_contract_revision, object_name, storage_key, \
                content_type, size_bytes, digest_sha256, revision, created_at, updated_at\
             ) VALUES (?1, ?2, 2, 'banner.png', 'k2', 'image/png', 200, 'sha256:9999999999999999999999999999999999999999999999999999999999999999', 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![tenant_id.to_string().into(), module_slug.into()],
        ))
        .await
        .expect("insert conflicting target object");

    let obj_service = ArtifactDataObjectMigrationService::new(database.clone());

    let req = ArtifactDataObjectMigrationRequest {
        tenant_id,
        module_slug: module_slug.to_string(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        context: command_context(tenant_id),
        reason: "conflict test".to_string(),
    };

    let err = obj_service
        .migrate_objects(req)
        .await
        .expect_err("must abort on conflict");
    assert_eq!(
        err,
        ArtifactDataObjectMigrationError::TargetObjectConflict("banner.png".to_string())
    );

    // 3. Test reconcile_stale_intents
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_object_copy_operations (\
                operation_id, tenant_id, module_slug, source_contract_revision, target_contract_revision, \
                inventory_manifest_digest, object_name, storage_key, digest_sha256, size_bytes, status, \
                actor_id, trace_id, correlation_id, idempotency_key, reason, created_at\
             ) VALUES (?1, ?2, ?3, 1, 2, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', \
                'stale.bin', 'k3', 'sha256:0000000000000000000000000000000000000000000000000000000000000000', 50, 'intent', \
                ?4, 'trace', ?5, ?6, 'crashed intent', '2026-09-01T00:00:00Z')",
            vec![
                Uuid::new_v4().to_string().into(),
                tenant_id.to_string().into(),
                module_slug.into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("insert stale intent");

    let reconciled = obj_service
        .reconcile_stale_intents(tenant_id, module_slug)
        .await
        .expect("reconcile succeeds");
    assert_eq!(reconciled, 1);
}
