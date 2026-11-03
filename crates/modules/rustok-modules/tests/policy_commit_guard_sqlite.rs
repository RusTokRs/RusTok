use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRegistry, RusToKModule};
use rustok_modules::{
    ModuleLifecycleDbWriter, ModuleOperationJournal, ModuleOperationRecordOutcome,
    ModuleOperationRequest, ModuleOperationStoreError, ModulePolicyRevisionApplyOutcome,
    ModulePolicyRevisionConsumerError, ModulePolicyRevisionCursorState,
    ModulePolicyRevisionTransition, ModulesModule, SeaOrmModulePolicyRevisionConsumer,
    StaticTenantLifecycleClaim, StaticTenantLifecycleStore, StaticTenantLifecycleStoreError,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use sea_orm_migration::MigrationTrait;
use uuid::Uuid;

struct NotificationsTestModule;

impl MigrationSource for NotificationsTestModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for NotificationsTestModule {
    fn slug(&self) -> &'static str {
        "notifications"
    }

    fn name(&self) -> &'static str {
        "Notifications test"
    }

    fn description(&self) -> &'static str {
        "Transaction-bound effective policy test module"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }
}

#[tokio::test]
async fn sqlite_aggregate_writes_follow_parent_uuid_and_fence_claims() {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    for sql in [
        "PRAGMA foreign_keys = ON",
        "CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants",
        "CREATE TABLE module_static_tenant_lifecycle (tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, revision INTEGER NOT NULL, \
         active_idempotency_key BLOB, active_claimed_at TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
         updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, module_slug), \
         FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
    ] {
        db.execute_unprepared(sql).await.unwrap();
    }
    let native = Uuid::new_v4();
    let text = Uuid::new_v4();
    let uppercase = Uuid::new_v4();
    for parent in [
        native.into(),
        text.to_string().into(),
        uppercase.to_string().to_uppercase().into(),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![parent],
        ))
        .await
        .unwrap();
    }
    for tenant in [native, text, uppercase] {
        let first = Uuid::new_v4();
        assert!(matches!(
            StaticTenantLifecycleStore::claim(&db, tenant, "notifications", 0, first)
                .await
                .unwrap(),
            StaticTenantLifecycleClaim::Acquired(_)
        ));
        assert!(
            StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 0, first)
                .await
                .unwrap()
        );
        assert!(matches!(
            StaticTenantLifecycleStore::claim(&db, tenant, "notifications", 0, Uuid::new_v4())
                .await,
            Err(StaticTenantLifecycleStoreError::OperationInProgress { .. })
        ));
        let duplicate: sea_orm::Value = if tenant == native {
            tenant.to_string().into()
        } else {
            tenant.into()
        };
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![duplicate.clone()],
        ))
        .await
        .unwrap();
        assert!(
            !StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 0, first)
                .await
                .unwrap()
        );
        assert!(
            StaticTenantLifecycleStore::advance(&db, tenant, "notifications", 0, first)
                .await
                .is_err()
        );
        assert!(
            StaticTenantLifecycleStore::release(&db, tenant, "notifications", first)
                .await
                .is_err()
        );
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM tenants WHERE id = ?1",
            vec![duplicate],
        ))
        .await
        .unwrap();
        assert!(
            StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 0, first)
                .await
                .unwrap()
        );
        // Legacy active claim keys can also be native UUID blobs.
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE module_static_tenant_lifecycle SET active_idempotency_key = ?1 WHERE module_slug = 'notifications' AND tenant_id COLLATE NOCASE IN (?2, ?3)",
            vec![first.into(), tenant.into(), tenant.to_string().into()])).await.unwrap();
        assert!(
            StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 0, first)
                .await
                .unwrap()
        );
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE module_static_tenant_lifecycle SET active_idempotency_key = ?1 WHERE module_slug = 'notifications' AND tenant_id COLLATE NOCASE IN (?2, ?3)",
            vec![first.to_string().to_uppercase().into(), tenant.into(), tenant.to_string().into()])).await.unwrap();
        assert!(
            StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 0, first)
                .await
                .unwrap()
        );
        assert_eq!(
            StaticTenantLifecycleStore::advance(&db, tenant, "notifications", 0, first)
                .await
                .unwrap(),
            1
        );
        StaticTenantLifecycleStore::release(&db, tenant, "notifications", first)
            .await
            .unwrap();
        assert!(matches!(
            StaticTenantLifecycleStore::claim(&db, tenant, "notifications", 0, Uuid::new_v4())
                .await,
            Err(StaticTenantLifecycleStoreError::RevisionConflict { .. })
        ));
        let old = Uuid::new_v4();
        let replacement = Uuid::new_v4();
        StaticTenantLifecycleStore::claim(&db, tenant, "notifications", 1, old)
            .await
            .unwrap();
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE module_static_tenant_lifecycle SET active_idempotency_key = ?1, active_claimed_at = '2000-01-01T00:00:00Z' WHERE module_slug = 'notifications' AND tenant_id COLLATE NOCASE IN (?2, ?3)",
            vec![old.into(), tenant.into(), tenant.to_string().into()])).await.unwrap();
        assert!(matches!(
            StaticTenantLifecycleStore::claim(&db, tenant, "notifications", 1, replacement)
                .await
                .unwrap(),
            StaticTenantLifecycleClaim::Acquired(_)
        ));
        assert!(
            !StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 1, old)
                .await
                .unwrap()
        );
        assert!(
            StaticTenantLifecycleStore::advance(&db, tenant, "notifications", 1, old)
                .await
                .is_err()
        );
        assert_eq!(
            StaticTenantLifecycleStore::advance(&db, tenant, "notifications", 1, replacement)
                .await
                .unwrap(),
            2
        );
        StaticTenantLifecycleStore::release(&db, tenant, "notifications", replacement)
            .await
            .unwrap();
        assert_eq!(
            StaticTenantLifecycleStore::snapshot(&db, tenant, "notifications")
                .await
                .unwrap()
                .revision,
            2
        );
    }
    for form in 0..3 {
        let tenant = Uuid::new_v4();
        let stored = match form {
            0 => tenant.simple().to_string(),
            1 => format!("URN:UUID:{tenant}"),
            _ => format!("{{{tenant}}}"),
        };
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![stored.into()],
        ))
        .await
        .unwrap();
        let claim = Uuid::new_v4();
        assert!(matches!(
            StaticTenantLifecycleStore::claim(&db, tenant, "notifications", 0, claim)
                .await
                .unwrap(),
            StaticTenantLifecycleClaim::Acquired(_)
        ));
        assert!(
            StaticTenantLifecycleStore::heartbeat(&db, tenant, "notifications", 0, claim)
                .await
                .unwrap()
        );
        assert_eq!(
            StaticTenantLifecycleStore::advance(&db, tenant, "notifications", 0, claim)
                .await
                .unwrap(),
            1
        );
        StaticTenantLifecycleStore::release(&db, tenant, "notifications", claim)
            .await
            .unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![tenant.into()],
        ))
        .await
        .unwrap();
        assert!(
            StaticTenantLifecycleStore::claim(&db, tenant, "other", 0, Uuid::new_v4())
                .await
                .is_err()
        );
    }
    let absent = Uuid::new_v4();
    assert!(matches!(
        StaticTenantLifecycleStore::claim(&db, absent, "notifications", 0, Uuid::new_v4()).await,
        Err(StaticTenantLifecycleStoreError::InconsistentState(_))
    ));
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?1)",
        vec![native.to_string().into()],
    ))
    .await
    .unwrap();
    assert!(
        StaticTenantLifecycleStore::claim(&db, native, "other", 0, Uuid::new_v4())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn sqlite_journal_detects_legacy_identity_with_real_foreign_keys() {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    for sql in [
        "PRAGMA foreign_keys = ON",
        "CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants",
        "CREATE TABLE tenant_modules (tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
        "CREATE TABLE module_static_tenant_lifecycle (tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
        "CREATE TABLE module_policy_revision_cursors (tenant_id BLOB NOT NULL, consumer_key TEXT NOT NULL)",
        "CREATE TABLE module_operations (id BLOB PRIMARY KEY NOT NULL, tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, \
         requested_enabled BOOLEAN NOT NULL, previous_effective_enabled BOOLEAN NOT NULL, status TEXT NOT NULL, \
         requested_by TEXT, trace_id TEXT, correlation_id TEXT, idempotency_key BLOB, expected_revision INTEGER, \
         recovery_source_id TEXT, error_message TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
         updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
        "CREATE UNIQUE INDEX journal_idempotency ON module_operations (tenant_id, idempotency_key) WHERE idempotency_key IS NOT NULL",
    ] {
        db.execute_unprepared(sql).await.unwrap();
    }
    let native = Uuid::new_v4();
    let legacy = Uuid::new_v4();
    for value in [native.into(), legacy.to_string().to_uppercase().into()] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![value],
        ))
        .await
        .unwrap();
    }
    let request = |tenant_id, key| ModuleOperationRequest {
        tenant_id,
        module_slug: "notifications".into(),
        requested_enabled: true,
        previous_effective_enabled: false,
        requested_by: None,
        trace_id: None,
        correlation_id: "trace".into(),
        idempotency_key: Some(key),
        recovery_source_id: None,
        expected_revision: Some(0),
    };
    for form in 0..3 {
        let tenant = Uuid::new_v4();
        let stored = match form {
            0 => tenant.simple().to_string(),
            1 => format!("URN:UUID:{tenant}"),
            _ => format!("{{{tenant}}}"),
        };
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![stored.into()],
        ))
        .await
        .unwrap();
        let command = request(tenant, Uuid::new_v4());
        let first = ModuleOperationJournal::record_idempotent(&db, command.clone())
            .await
            .unwrap();
        let replay = ModuleOperationJournal::record_idempotent(&db, command)
            .await
            .unwrap();
        let id = match (first, replay) {
            (
                ModuleOperationRecordOutcome::Recorded(first),
                ModuleOperationRecordOutcome::Replayed(replay),
            ) if first.id == replay.id => first.id,
            _ => panic!("text UUID parent must preserve journal replay"),
        };
        assert_eq!(
            ModuleOperationJournal::find(&db, id)
                .await
                .unwrap()
                .unwrap()
                .tenant_id,
            tenant
        );
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![tenant.into()],
        ))
        .await
        .unwrap();
        assert!(matches!(
            ModuleOperationJournal::find(&db, id).await,
            Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
        ));
    }
    let missing_tenant = request(Uuid::new_v4(), Uuid::new_v4());
    assert!(matches!(
        ModuleOperationJournal::record_idempotent(&db, missing_tenant.clone()).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    assert!(matches!(
        ModuleOperationJournal::record(&db, missing_tenant).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    let native_request = request(native, Uuid::new_v4());
    let first = ModuleOperationJournal::record_idempotent(&db, native_request.clone())
        .await
        .unwrap();
    let replay = ModuleOperationJournal::record_idempotent(&db, native_request)
        .await
        .unwrap();
    let native_operation = match (first, replay) {
        (
            ModuleOperationRecordOutcome::Recorded(first),
            ModuleOperationRecordOutcome::Replayed(replay),
        ) if first.id == replay.id => first,
        _ => panic!("native tenant journal must replay the original operation"),
    };
    assert_eq!(
        ModuleOperationJournal::find(&db, native_operation.id)
            .await
            .unwrap()
            .unwrap()
            .tenant_id,
        native
    );
    let legacy_key = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, idempotency_key, expected_revision) VALUES (?1, ?2, 'notifications', TRUE, FALSE, 'validated', ?3, 0)",
        vec![Uuid::new_v4().into(), legacy.to_string().to_uppercase().into(), legacy_key.into()],
    )).await.unwrap();
    assert!(matches!(
        ModuleOperationJournal::record_idempotent(&db, request(legacy, legacy_key))
            .await
            .unwrap(),
        ModuleOperationRecordOutcome::Replayed(_)
    ));
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM module_operations WHERE tenant_id COLLATE NOCASE = ?1",
            vec![legacy.to_string().into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 1);
    let new_legacy_request = request(legacy, Uuid::new_v4());
    let legacy_insert = ModuleOperationJournal::record_idempotent(&db, new_legacy_request.clone())
        .await
        .unwrap();
    let legacy_replay = ModuleOperationJournal::record_idempotent(&db, new_legacy_request)
        .await
        .unwrap();
    let legacy_operation = match (legacy_insert, legacy_replay) {
        (
            ModuleOperationRecordOutcome::Recorded(first),
            ModuleOperationRecordOutcome::Replayed(replay),
        ) if first.id == replay.id => first,
        _ => panic!("text tenant journal must replay the original operation"),
    };
    assert_eq!(
        ModuleOperationJournal::find(&db, legacy_operation.id)
            .await
            .unwrap()
            .unwrap()
            .tenant_id,
        legacy
    );
    ModuleOperationJournal::mark_failed(&db, legacy_operation.id, "test failure")
        .await
        .unwrap();
    let failures = ModuleOperationJournal::failed_for_tenant(&db, legacy, Some("notifications"))
        .await
        .unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].id, legacy_operation.id);
    assert!(
        ModuleOperationJournal::failed_for_tenant(&db, native, None)
            .await
            .unwrap()
            .is_empty()
    );
    let migration = ModulesModule
        .migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20261102_000090_journal_uuid_uniqueness")
        .unwrap();
    migration
        .up(&sea_orm_migration::SchemaManager::new(&db))
        .await
        .unwrap();
    let representations = ModulesModule
        .migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20261103_000091_tenant_identity_indexes")
        .unwrap();
    representations
        .up(&sea_orm_migration::SchemaManager::new(&db))
        .await
        .unwrap();
    let collision = db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, idempotency_key, expected_revision) VALUES (?1, ?2, 'notifications', TRUE, FALSE, 'validated', ?3, 0)",
        vec![Uuid::new_v4().into(), legacy.to_string().to_uppercase().into(), format!("{{{legacy_key}}}").into()],
    )).await.expect_err("all supported text representations must collide");
    assert!(collision.to_string().to_lowercase().contains("unique"));
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?1)",
        vec![legacy.into()],
    ))
    .await
    .unwrap();
    assert!(matches!(
        ModuleOperationJournal::find(&db, legacy_operation.id).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    assert!(matches!(
        ModuleOperationJournal::record_idempotent(&db, request(legacy, Uuid::new_v4())).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    assert!(matches!(
        ModuleOperationJournal::record(&db, request(legacy, Uuid::new_v4())).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    assert!(matches!(
        ModuleOperationJournal::failed_for_tenant(&db, legacy, Some("unrelated")).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    let collision = db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, idempotency_key, expected_revision) VALUES (?1, ?2, 'notifications', TRUE, FALSE, 'validated', ?3, 0)",
        vec![Uuid::new_v4().into(), legacy.into(), legacy_key.to_string().into()],
    )).await.expect_err("the migrated index must reject mixed-encoding writers");
    assert!(collision.to_string().to_lowercase().contains("unique"));
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO module_operations (id, tenant_id, module_slug, requested_enabled, previous_effective_enabled, status, idempotency_key, expected_revision) VALUES (?1, ?2, 'other', TRUE, FALSE, 'validated', ?3, 0)",
        vec![Uuid::new_v4().into(), native.to_string().into(), Uuid::new_v4().into()],
    )).await.unwrap();
    assert!(matches!(
        ModuleOperationJournal::record_idempotent(&db, request(native, Uuid::new_v4())).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    assert!(matches!(
        ModuleOperationJournal::record(&db, request(native, Uuid::new_v4())).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    assert!(matches!(
        ModuleOperationJournal::find(&db, native_operation.id).await,
        Err(ModuleOperationStoreError::UnverifiableSqliteIdentity)
    ));
    db.execute_unprepared("DROP VIEW tenant_identity_keys")
        .await
        .unwrap();
    assert!(
        ModuleOperationJournal::record_idempotent(&db, request(native, Uuid::new_v4()))
            .await
            .is_err()
    );
    assert!(
        ModuleOperationJournal::record(&db, request(native, Uuid::new_v4()))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn static_policy_resolves_tenant_override_under_lifecycle_cursor_lock() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());

    let enabled_txn = db.begin().await.expect("enabled policy transaction");
    let enabled = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &enabled_txn,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".to_string()],
        )
        .await
        .expect("default-enabled policy should resolve");
    assert!(enabled.contains("notifications"));
    let enabled_revision = enabled.policy_revision().to_string();
    enabled_txn.commit().await.expect("enabled policy commit");

    let disabled_txn = db.begin().await.expect("disabled policy transaction");
    disabled_txn
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?1, ?2, ?3)"
                .to_string(),
            vec![
                tenant_id.to_string().into(),
                "notifications".to_string().into(),
                false.into(),
            ],
        ))
        .await
        .expect("disabled tenant override should persist in transaction");
    let disabled = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &disabled_txn,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".to_string()],
        )
        .await
        .expect("transaction-bound disabled policy should resolve");
    assert!(!disabled.contains("notifications"));
    assert_ne!(disabled.policy_revision(), enabled_revision);
    disabled_txn.commit().await.expect("disabled policy commit");

    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT current_revision FROM module_policy_revision_cursors WHERE tenant_id = ?1 AND consumer_key = ?2"
                .to_string(),
            vec![
                tenant_id.to_string().into(),
                "module.lifecycle".to_string().into(),
            ],
        ))
        .await
        .expect("cursor query should succeed")
        .expect("cursor row should exist");
    let current_revision: Option<String> = row
        .try_get("", "current_revision")
        .expect("cursor revision should decode");
    assert!(
        current_revision.is_none(),
        "commit guard locks but never advances the lifecycle owner cursor"
    );
}

#[tokio::test]
async fn static_policy_reads_native_uuid_and_rejects_duplicate_identity() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?1, ?2, ?3)",
        vec![tenant_id.into(), "notifications".into(), false.into()],
    ))
    .await
    .unwrap();
    let transaction = db.begin().await.unwrap();
    let policy = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &transaction,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".into()],
        )
        .await
        .unwrap();
    assert!(!policy.contains("notifications"));
    transaction.rollback().await.unwrap();

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?1, ?2, ?3)",
        vec![
            tenant_id.to_string().into(),
            "notifications".into(),
            true.into(),
        ],
    ))
    .await
    .unwrap();
    let transaction = db.begin().await.unwrap();
    let result = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &transaction,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".into()],
        )
        .await;
    assert!(result.is_err(), "ambiguous tenant state must fail closed");
    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn sqlite_policy_honors_uppercase_tenant_key_and_rejects_duplicate_parent() {
    let db = setup().await;
    let tenant_id = Uuid::parse_str("abcdefab-cdef-4abc-8def-abcdefabcdef").unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?1, 'notifications', FALSE)",
        vec![tenant_id.to_string().to_uppercase().into()],
    )).await.unwrap();
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
    let tx = db.begin().await.unwrap();
    let policy = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &tx,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".into()],
        )
        .await
        .unwrap();
    assert!(!policy.contains("notifications"));
    tx.rollback().await.unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?1)",
        vec![tenant_id.into()],
    ))
    .await
    .unwrap();
    let tx = db.begin().await.unwrap();
    assert!(
        consumer
            .lock_and_resolve_static_policy_in_transaction(
                &tx,
                tenant_id,
                "module.lifecycle",
                &registry,
                vec!["notifications".into()],
            )
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn sqlite_policy_honors_all_supported_tenant_uuid_forms() {
    for form in 0..3 {
        let db = setup().await;
        let tenant_id = Uuid::new_v4();
        let stored = match form {
            0 => tenant_id.simple().to_string(),
            1 => format!("URN:UUID:{tenant_id}"),
            _ => format!("{{{tenant_id}}}"),
        };
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?1, 'notifications', FALSE)",
            vec![stored.into()],
        )).await.unwrap();
        let registry = ModuleRegistry::new().register(NotificationsTestModule);
        let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
        let tx = db.begin().await.unwrap();
        let policy = consumer
            .lock_and_resolve_static_policy_in_transaction(
                &tx,
                tenant_id,
                "module.lifecycle",
                &registry,
                vec!["notifications".into()],
            )
            .await
            .unwrap();
        assert!(!policy.contains("notifications"));
        tx.rollback().await.unwrap();
    }
}

#[tokio::test]
async fn sqlite_policy_fails_closed_when_tenant_projection_is_missing() {
    let db = setup().await;
    db.execute_unprepared("DROP VIEW tenant_identity_keys")
        .await
        .unwrap();
    let tenant_id = Uuid::new_v4();
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let writer = ModuleLifecycleDbWriter::new(db.clone(), &registry, vec![]);
    assert!(
        writer
            .tenant_override_snapshots(tenant_id, 1)
            .await
            .is_err()
    );
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
    let transaction = db.begin().await.unwrap();
    assert!(
        consumer
            .lock_and_resolve_static_policy_in_transaction(
                &transaction,
                tenant_id,
                "module.lifecycle",
                &registry,
                vec!["notifications".into()],
            )
            .await
            .is_err()
    );
    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn sqlite_real_foreign_keys_preserve_native_and_text_tenant_isolation() {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    for sql in [
        "PRAGMA foreign_keys = ON",
        "CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants",
        "CREATE TABLE tenant_modules (tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, enabled BOOLEAN, settings JSON NOT NULL, \
         PRIMARY KEY (tenant_id, module_slug), FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
        "CREATE TABLE module_static_tenant_lifecycle (tenant_id BLOB NOT NULL, module_slug TEXT NOT NULL, revision INTEGER NOT NULL, \
         active_idempotency_key TEXT, PRIMARY KEY (tenant_id, module_slug), FOREIGN KEY (tenant_id) REFERENCES tenants(id))",
        "CREATE TABLE module_policy_revision_cursors (tenant_id TEXT NOT NULL, consumer_key TEXT NOT NULL, current_revision TEXT, \
         current_sequence INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, consumer_key))",
    ] {
        db.execute_unprepared(sql).await.unwrap();
    }
    let native_tenant = Uuid::new_v4();
    let text_tenant = Uuid::new_v4();
    for key in [native_tenant.into(), text_tenant.to_string().into()] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![key],
        ))
        .await
        .unwrap();
    }
    for (key, enabled) in [
        (native_tenant.into(), false),
        (text_tenant.to_string().into(), true),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES (?1, 'notifications', ?2, '{}')",
            vec![key, enabled.into()],
        ))
        .await
        .unwrap();
    }
    for (key, revision) in [
        (native_tenant.into(), 7_i64),
        (text_tenant.to_string().into(), 3_i64),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_static_tenant_lifecycle (tenant_id, module_slug, revision) VALUES (?1, 'notifications', ?2)",
            vec![key, revision.into()],
        )).await.unwrap();
    }
    for (tenant, revision) in [(native_tenant, 7), (text_tenant, 3)] {
        assert_eq!(
            StaticTenantLifecycleStore::snapshot(&db, tenant, "notifications")
                .await
                .unwrap()
                .revision,
            revision
        );
        let snapshots = StaticTenantLifecycleStore::snapshots(
            &db,
            tenant,
            ["notifications".to_string(), "missing".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(snapshots["notifications"].revision, revision);
        assert_eq!(snapshots["missing"].revision, 0);
    }
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let writer = ModuleLifecycleDbWriter::new(db.clone(), &registry, vec![]);
    assert_eq!(
        writer
            .tenant_override_snapshots(native_tenant, 1)
            .await
            .unwrap()[0]
            .enabled,
        Some(false)
    );
    assert_eq!(
        writer
            .tenant_override_snapshots(text_tenant, 1)
            .await
            .unwrap()[0]
            .enabled,
        Some(true)
    );
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
    for (tenant, expected) in [(native_tenant, false), (text_tenant, true)] {
        let transaction = db.begin().await.unwrap();
        let policy = consumer
            .lock_and_resolve_static_policy_in_transaction(
                &transaction,
                tenant,
                "module.lifecycle",
                &registry,
                Vec::<String>::new(),
            )
            .await
            .unwrap();
        assert_eq!(policy.contains("notifications"), expected);
        transaction.rollback().await.unwrap();
    }
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?1)",
        vec![native_tenant.to_string().into()],
    ))
    .await
    .unwrap();
    assert!(
        writer
            .tenant_override_snapshots(native_tenant, 1)
            .await
            .is_err()
    );
    assert!(
        StaticTenantLifecycleStore::snapshot(&db, native_tenant, "missing")
            .await
            .is_err()
    );
    assert!(
        StaticTenantLifecycleStore::snapshots(&db, native_tenant, ["notifications".to_string()])
            .await
            .is_err()
    );
    let transaction = db.begin().await.unwrap();
    assert!(
        consumer
            .lock_and_resolve_static_policy_in_transaction(
                &transaction,
                native_tenant,
                "module.lifecycle",
                &registry,
                Vec::<String>::new(),
            )
            .await
            .is_err()
    );
    transaction.rollback().await.unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "DELETE FROM tenants WHERE id = ?1",
        vec![native_tenant.to_string().into()],
    ))
    .await
    .unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO module_static_tenant_lifecycle (tenant_id, module_slug, revision) VALUES (?1, 'orphan', 1)",
        vec![native_tenant.to_string().into()],
    )).await.unwrap();
    assert!(
        StaticTenantLifecycleStore::snapshot(&db, native_tenant, "missing")
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL"]
async fn postgres_policy_and_operator_reads_preserve_uuid_contract() {
    let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL")
        .expect("RUSTOK_MODULES_TEST_DATABASE_URL is required");
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    for sql in [
        "CREATE TEMP TABLE tenant_modules (id UUID NOT NULL, tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, \
         enabled BOOLEAN, settings JSONB NOT NULL, PRIMARY KEY (tenant_id, module_slug))",
        "CREATE TEMP TABLE module_static_tenant_lifecycle (tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, \
         revision BIGINT NOT NULL, active_idempotency_key UUID, PRIMARY KEY (tenant_id, module_slug))",
        "CREATE TEMP TABLE module_operations (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, \
         requested_enabled BOOLEAN NOT NULL, previous_effective_enabled BOOLEAN NOT NULL, status TEXT NOT NULL, \
         requested_by TEXT, trace_id TEXT, correlation_id TEXT, idempotency_key UUID, expected_revision BIGINT, \
         recovery_source_id TEXT, error_message TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
         updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (tenant_id, idempotency_key))",
        "CREATE TEMP TABLE module_policy_revision_cursors (tenant_id UUID NOT NULL, consumer_key TEXT NOT NULL, \
         current_revision TEXT, current_sequence BIGINT NOT NULL DEFAULT 0, updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
         PRIMARY KEY (tenant_id, consumer_key))",
    ] {
        db.execute_unprepared(sql).await.unwrap();
    }
    let tenant_id = Uuid::new_v4();
    let journal_request = ModuleOperationRequest {
        tenant_id,
        module_slug: "notifications".into(),
        requested_enabled: true,
        previous_effective_enabled: false,
        requested_by: None,
        trace_id: None,
        correlation_id: "trace".into(),
        idempotency_key: Some(Uuid::new_v4()),
        recovery_source_id: None,
        expected_revision: Some(0),
    };
    let first = ModuleOperationJournal::record_idempotent(&db, journal_request.clone())
        .await
        .unwrap();
    let replay = ModuleOperationJournal::record_idempotent(&db, journal_request)
        .await
        .unwrap();
    let journal_operation = match (first, replay) {
        (
            ModuleOperationRecordOutcome::Recorded(first),
            ModuleOperationRecordOutcome::Replayed(replay),
        ) if first.id == replay.id => first,
        _ => panic!("journal must replay the original operation"),
    };
    ModuleOperationJournal::mark_failed(&db, journal_operation.id, "test failure")
        .await
        .unwrap();
    let failures = ModuleOperationJournal::failed_for_tenant(&db, tenant_id, Some("notifications"))
        .await
        .unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].id, journal_operation.id);
    assert!(
        ModuleOperationJournal::failed_for_tenant(&db, Uuid::new_v4(), None)
            .await
            .unwrap()
            .is_empty()
    );
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) VALUES ($1, $2, 'notifications', FALSE, '{\"value\":3}'::jsonb)",
        vec![Uuid::new_v4().into(), tenant_id.into()],
    ))
    .await
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO module_static_tenant_lifecycle (tenant_id, module_slug, revision) VALUES ($1, 'notifications', 4)",
        vec![tenant_id.into()],
    )).await.unwrap();
    assert_eq!(
        StaticTenantLifecycleStore::snapshot(&db, tenant_id, "notifications")
            .await
            .unwrap()
            .revision,
        4
    );
    let registry = ModuleRegistry::new().register(NotificationsTestModule);
    let writer = ModuleLifecycleDbWriter::new(db.clone(), &registry, vec![]);
    let snapshots = writer
        .tenant_override_snapshots(tenant_id, 1)
        .await
        .unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].enabled, Some(false));
    assert_eq!(snapshots[0].settings["value"], 3);
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
    let transaction = db.begin().await.unwrap();
    let policy = consumer
        .lock_and_resolve_static_policy_in_transaction(
            &transaction,
            tenant_id,
            "module.lifecycle",
            &registry,
            vec!["notifications".into()],
        )
        .await
        .unwrap();
    assert!(!policy.contains("notifications"));
    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn lifecycle_cursor_sequences_exact_successors_and_requires_explicit_resync() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let consumer = SeaOrmModulePolicyRevisionConsumer::new(db);
    let first_revision = format!("sha256:{}", "a".repeat(64));
    let second_revision = format!("sha256:{}", "b".repeat(64));
    let third_revision = format!("sha256:{}", "c".repeat(64));
    let fourth_revision = format!("sha256:{}", "d".repeat(64));

    let initial_state = consumer
        .cursor_state(tenant_id, "module.lifecycle")
        .await
        .expect("uninitialized cursor should resolve");
    assert_eq!(
        initial_state,
        ModulePolicyRevisionCursorState {
            current_revision: None,
            current_sequence: 0,
        }
    );

    let first = ModulePolicyRevisionTransition {
        previous_revision: None,
        next_revision: first_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &first)
            .await
            .expect("first successor should advance"),
        ModulePolicyRevisionApplyOutcome::Applied
    );
    assert_eq!(
        consumer
            .cursor_state(tenant_id, "module.lifecycle")
            .await
            .expect("first cursor state should resolve"),
        ModulePolicyRevisionCursorState {
            current_revision: Some(first_revision.clone()),
            current_sequence: 1,
        }
    );

    let second = ModulePolicyRevisionTransition {
        previous_revision: Some(first_revision.clone()),
        next_revision: second_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &second)
            .await
            .expect("matching successor should advance"),
        ModulePolicyRevisionApplyOutcome::Applied
    );
    let second_state = consumer
        .cursor_state(tenant_id, "module.lifecycle")
        .await
        .expect("second cursor state should resolve");
    assert_eq!(
        second_state,
        ModulePolicyRevisionCursorState {
            current_revision: Some(second_revision.clone()),
            current_sequence: 2,
        }
    );

    // Resyncing to the revision the cursor already holds validates the
    // exact expected state without advancing the sequence. The lifecycle
    // writer relies on this no-op form to prove that the cursor did not
    // move while it derived a policy snapshot.
    assert_eq!(
        consumer
            .resync_if_current(
                tenant_id,
                "module.lifecycle",
                &second_state,
                &second_revision,
            )
            .await
            .expect("same-revision resync should be a no-op"),
        second_state
    );

    let gap = ModulePolicyRevisionTransition {
        previous_revision: Some(first_revision),
        next_revision: third_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &gap)
            .await
            .expect("gap should be reported without storage failure"),
        ModulePolicyRevisionApplyOutcome::ResyncRequired
    );
    assert_eq!(
        consumer
            .cursor_state(tenant_id, "module.lifecycle")
            .await
            .expect("gap must leave cursor state unchanged"),
        second_state
    );

    let stale_resync = consumer
        .resync_if_current(
            tenant_id,
            "module.lifecycle",
            &initial_state,
            &third_revision,
        )
        .await;
    assert!(matches!(
        stale_resync,
        Err(ModulePolicyRevisionConsumerError::CursorStateConflict {
            expected_sequence: 0,
            current_sequence: 2,
        })
    ));

    let resynced_state = consumer
        .resync_if_current(
            tenant_id,
            "module.lifecycle",
            &second_state,
            &third_revision,
        )
        .await
        .expect("explicit full-snapshot resync should advance once");
    assert_eq!(
        resynced_state,
        ModulePolicyRevisionCursorState {
            current_revision: Some(third_revision.clone()),
            current_sequence: 3,
        }
    );

    let fourth = ModulePolicyRevisionTransition {
        previous_revision: Some(third_revision),
        next_revision: fourth_revision.clone(),
    };
    assert_eq!(
        consumer
            .apply(tenant_id, "module.lifecycle", &fourth)
            .await
            .expect("successor after explicit resync should advance"),
        ModulePolicyRevisionApplyOutcome::Applied
    );
    assert_eq!(
        consumer
            .cursor_state(tenant_id, "module.lifecycle")
            .await
            .expect("fourth cursor state should resolve"),
        ModulePolicyRevisionCursorState {
            current_revision: Some(fourth_revision),
            current_sequence: 4,
        }
    );
}

#[tokio::test]
async fn sqlite_cursor_preserves_revision_across_uuid_encodings_and_rejects_duplicates() {
    for form in 0..5 {
        let db = setup().await;
        let tenant_id = Uuid::new_v4();
        let stored: sea_orm::Value = match form {
            0 => tenant_id.into(),
            1 => tenant_id.to_string().to_uppercase().into(),
            2 => tenant_id.simple().to_string().into(),
            3 => format!("URN:UUID:{tenant_id}").into(),
            _ => format!("{{{tenant_id}}}").into(),
        };
        let original = format!("sha256:{}", "a".repeat(64));
        let next = format!("sha256:{}", "b".repeat(64));
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO module_policy_revision_cursors (tenant_id, consumer_key, current_revision, current_sequence) VALUES (?1, 'worker', ?2, 5)",
            vec![stored, original.clone().into()])).await.unwrap();
        let consumer = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
        assert_eq!(
            consumer.cursor_state(tenant_id, "worker").await.unwrap(),
            ModulePolicyRevisionCursorState {
                current_revision: Some(original.clone()),
                current_sequence: 5
            }
        );
        assert_eq!(
            consumer
                .apply(
                    tenant_id,
                    "worker",
                    &ModulePolicyRevisionTransition {
                        previous_revision: Some(original),
                        next_revision: next.clone(),
                    }
                )
                .await
                .unwrap(),
            ModulePolicyRevisionApplyOutcome::Applied
        );
        assert_eq!(
            consumer.cursor_state(tenant_id, "worker").await.unwrap(),
            ModulePolicyRevisionCursorState {
                current_revision: Some(next),
                current_sequence: 6
            }
        );
        let count = db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM module_policy_revision_cursors",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "count")
            .unwrap();
        assert_eq!(count, 1);
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO module_policy_revision_cursors (tenant_id, consumer_key, current_revision, current_sequence) VALUES (?1, 'worker', NULL, 0)",
            vec![if form == 0 { tenant_id.to_string().into() } else { tenant_id.into() }])).await.unwrap();
        assert!(consumer.cursor_state(tenant_id, "worker").await.is_err());
    }
}

async fn setup() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:module_policy_commit_guard_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("policy guard sqlite database");
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants;
        CREATE TABLE tenant_modules (
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL,
            PRIMARY KEY (tenant_id, module_slug)
        );
        CREATE TRIGGER fixture_tenant AFTER INSERT ON tenant_modules BEGIN
            INSERT OR IGNORE INTO tenants (id) VALUES (NEW.tenant_id);
        END;
        CREATE TABLE module_policy_revision_cursors (
            tenant_id TEXT NOT NULL,
            consumer_key TEXT NOT NULL,
            current_revision TEXT NULL,
            current_sequence INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, consumer_key)
        );
        "#,
    )
    .await
    .expect("policy guard schema");
    db
}
