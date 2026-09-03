//! Integration tests for crash-safe cross-revision artifact data copier and preflight classification.

use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataCopyError, ArtifactDataCrossRevisionCopier, CrossRevisionDataCopyRequest,
    MigrationPreflightInput, ModuleCommandContext, ModulesModule, UpdateMode,
    evaluate_migration_preflight,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

fn command_context(tenant_id: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: Some(tenant_id),
        idempotency_key: Uuid::new_v4(),
        trace_id: "test:cross-revision-copy".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

#[tokio::test]
async fn test_preflight_classifies_cross_revision_data_as_maintenance() {
    let input = MigrationPreflightInput {
        operation_id: Uuid::new_v4(),
        module_slug: "inventory".to_string(),
        source_schema_digest: "sha256:src".to_string(),
        target_schema_digest: "sha256:tgt".to_string(),
        migration_plan_digest: "sha256:plan".to_string(),
        is_additive_safe: true,
        migration_reasons: vec![],
        settings_guard_installed: true,
        has_irreversible_external_effects: false,
        requires_cross_revision_data_copy: true,
    };

    let receipt = evaluate_migration_preflight(input);
    assert_eq!(receipt.mode, UpdateMode::Maintenance);
    assert!(
        receipt
            .denial_reasons
            .iter()
            .any(|r| r.contains("maintenance-only"))
    );
}

#[tokio::test]
async fn test_cross_revision_data_copier_paged_copy_and_idempotency() {
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
    let module_slug = "catalog".to_string();

    // 1. Create source and target namespaces
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at) VALUES (?1, ?2, 1, 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![tenant_id.to_string().into(), module_slug.clone().into()],
        ))
        .await
        .expect("insert source namespace");

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at) VALUES (?1, ?2, 2, 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![tenant_id.to_string().into(), module_slug.clone().into()],
        ))
        .await
        .expect("insert target namespace");

    // 2. Insert 5 source records (keys: item_01 .. item_05)
    for i in 1..=5 {
        let key = format!("item_{:02}", i);
        let val = serde_json::json!({ "name": format!("Item {}", i), "stock": i * 10 });
        let val_bytes = serde_json::to_string(&val).unwrap();

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_data (tenant_id, module_slug, data_contract_revision, data_key, value, value_size_bytes, revision, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5, 1, '2026-09-01T00:00:00Z')",
                vec![
                    tenant_id.to_string().into(),
                    module_slug.clone().into(),
                    key.into(),
                    val_bytes.into(),
                    (val.to_string().len() as i64).into(),
                ],
            ))
            .await
            .expect("insert record");
    }

    let copier = ArtifactDataCrossRevisionCopier::new(database.clone());

    // 3. Copy Page 1 (size = 2): items 01 and 02
    let page1_context = command_context(tenant_id);
    let page1_req = CrossRevisionDataCopyRequest {
        tenant_id,
        module_slug: module_slug.clone(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        page_size: 2,
        page_cursor: None,
        context: page1_context.clone(),
        reason: "maintenance data migration page 1".to_string(),
    };
    let page1_res = copier.copy_page(page1_req).await.expect("page 1 succeeds");
    assert_eq!(page1_res.items_copied, 2);
    assert_eq!(page1_res.is_terminal_page, false);
    assert_eq!(page1_res.next_page_cursor, Some("item_02".to_string()));

    // Idempotent retry of Page 1 with same idempotency key
    let page1_retry = CrossRevisionDataCopyRequest {
        tenant_id,
        module_slug: module_slug.clone(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        page_size: 2,
        page_cursor: None,
        context: page1_context,
        reason: "maintenance data migration page 1 retry".to_string(),
    };
    let page1_retry_res = copier
        .copy_page(page1_retry)
        .await
        .expect("page 1 retry succeeds");
    assert_eq!(page1_retry_res.operation_id, page1_res.operation_id);
    assert_eq!(page1_retry_res.items_copied, 2);

    // 4. Copy Page 2 (size = 2): items 03 and 04
    let page2_req = CrossRevisionDataCopyRequest {
        tenant_id,
        module_slug: module_slug.clone(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        page_size: 2,
        page_cursor: page1_res.next_page_cursor,
        context: command_context(tenant_id),
        reason: "maintenance data migration page 2".to_string(),
    };
    let page2_res = copier.copy_page(page2_req).await.expect("page 2 succeeds");
    assert_eq!(page2_res.items_copied, 2);
    assert_eq!(page2_res.is_terminal_page, false);
    assert_eq!(page2_res.next_page_cursor, Some("item_04".to_string()));

    // 5. Copy Page 3 (size = 2): item 05 (terminal page)
    let page3_req = CrossRevisionDataCopyRequest {
        tenant_id,
        module_slug: module_slug.clone(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        page_size: 2,
        page_cursor: page2_res.next_page_cursor,
        context: command_context(tenant_id),
        reason: "maintenance data migration page 3".to_string(),
    };
    let page3_res = copier.copy_page(page3_req).await.expect("page 3 succeeds");
    assert_eq!(page3_res.items_copied, 1);
    assert_eq!(page3_res.is_terminal_page, true);
    assert_eq!(page3_res.next_page_cursor, None);

    // Verify all 5 records exist in target contract revision (2)
    let target_count_row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM module_artifact_data WHERE tenant_id = ?1 AND module_slug = ?2 AND data_contract_revision = 2",
            vec![tenant_id.to_string().into(), module_slug.clone().into()],
        ))
        .await
        .expect("count query")
        .unwrap();
    let target_count: i64 = target_count_row.try_get("", "count").unwrap();
    assert_eq!(target_count, 5);

    // 6. Test create-only conflict safety:
    // If a target key exists with a DIFFERENT value, copier refuses overwrite with TargetKeyConflict
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_data SET value = '{\"conflicting\": true}' WHERE tenant_id = ?1 AND module_slug = ?2 AND data_contract_revision = 2 AND data_key = 'item_01'",
            vec![tenant_id.to_string().into(), module_slug.clone().into()],
        ))
        .await
        .expect("tamper target key");

    let conflict_req = CrossRevisionDataCopyRequest {
        tenant_id,
        module_slug: module_slug.clone(),
        source_contract_revision: 1,
        target_contract_revision: 2,
        page_size: 2,
        page_cursor: None,
        context: command_context(tenant_id),
        reason: "conflict attempt".to_string(),
    };
    let conflict_err = copier
        .copy_page(conflict_req)
        .await
        .expect_err("should reject overwrite with conflict");
    assert_eq!(
        conflict_err,
        ArtifactDataCopyError::TargetKeyConflict("item_01".to_string())
    );

    // Verify target value was NOT overwritten
    let tampered_row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT CAST(value AS TEXT) AS value_text FROM module_artifact_data WHERE tenant_id = ?1 AND module_slug = ?2 AND data_contract_revision = 2 AND data_key = 'item_01'",
            vec![tenant_id.to_string().into(), module_slug.clone().into()],
        ))
        .await
        .expect("get tampered row")
        .unwrap();
    let tampered_val: String = tampered_row.try_get("", "value_text").unwrap();
    assert!(tampered_val.contains("conflicting"));

    // 7. Test reconcile_stale_intents
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_copy_operations (\
                operation_id, tenant_id, module_slug, source_contract_revision, target_contract_revision, \
                page_cursor, page_digest, items_count, status, actor_id, trace_id, correlation_id, \
                idempotency_key, reason, created_at\
             ) VALUES (?1, ?2, ?3, 1, 2, NULL, 'sha256:0000000000000000000000000000000000000000000000000000000000000000', 0, 'intent', ?4, 'trace', ?5, ?6, 'crashed page', '2026-09-02T00:00:00Z')",
            vec![
                Uuid::new_v4().to_string().into(),
                tenant_id.to_string().into(),
                module_slug.clone().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("insert stale intent");

    let reconciled = copier
        .reconcile_stale_intents(tenant_id, &module_slug)
        .await
        .expect("reconcile succeeds");
    assert_eq!(reconciled, 1);
}
