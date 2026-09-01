#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[tokio::test]
async fn index_module_registers_no_legacy_event_listeners() {
    use rustok_core::{ModuleEventListenerContext, ModuleRegistry, ModuleRuntimeExtensions};
    use sea_orm::Database;

    use crate::IndexModule;

    let registry = ModuleRegistry::new().register(IndexModule);
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    let extensions = ModuleRuntimeExtensions::default();
    let ctx = ModuleEventListenerContext {
        db,
        extensions: &extensions,
    };

    let handlers = registry.build_event_listeners(&ctx);
    assert!(
        handlers.is_empty(),
        "legacy Content/Product/Flex listeners must not return"
    );
}

#[test]
fn index_module_registers_canonical_storage_migrations() {
    use rustok_core::MigrationSource;

    use crate::IndexModule;

    let migrations = IndexModule.migrations();
    let names = migrations
        .iter()
        .map(|migration| migration.name())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "m20260727_000001_create_index_records",
            "m20260727_000002_create_index_delivery_state",
            "m20260727_000003_create_index_operations",
            "m20260803_000004_create_index_reconciliation_recovery",
        ]
    );

    let dependencies = IndexModule.migration_dependencies();
    assert_eq!(dependencies.len(), 4);
}

#[tokio::test]
async fn canonical_storage_migrations_round_trip_on_sqlite() {
    use std::collections::BTreeSet;

    use rustok_core::MigrationSource;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::SchemaManager;

    use crate::IndexModule;

    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should be enabled");
    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
        .await
        .expect("tenant fixture should be created");
    db.execute_unprepared(
        "INSERT INTO tenants (id) VALUES ('11111111-1111-1111-1111-111111111111')",
    )
    .await
    .expect("tenant fixture should be inserted");

    let manager = SchemaManager::new(&db);
    let migrations = IndexModule.migrations();
    for migration in &migrations {
        migration
            .up(&manager)
            .await
            .unwrap_or_else(|error| panic!("{} should apply: {error}", migration.name()));
    }

    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'index_%'"
                .to_owned(),
        ))
        .await
        .expect("index tables should be queryable");
    let tables = rows
        .iter()
        .map(|row| row.try_get("", "name").expect("table name should be text"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        tables,
        BTreeSet::from([
            "index_checkpoints".to_owned(),
            "index_consistency_findings".to_owned(),
            "index_entities".to_owned(),
            "index_inbox".to_owned(),
            "index_jobs".to_owned(),
            "index_links".to_owned(),
            "index_reconciliation_recovery_audits".to_owned(),
            "index_schemas".to_owned(),
        ])
    );

    let fingerprint = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    db.execute_unprepared(&format!(
        r#"INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog', 'product', 1, '{fingerprint}', '{{"fields":[]}}')"#
    ))
    .await
    .expect("schema row should be accepted");
    db.execute_unprepared(&format!(
        r#"INSERT INTO index_entities (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key, source_version, schema_fingerprint, payload) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog', 'product', 1, '22222222-2222-2222-2222-222222222222', 'en-US', 1, '{fingerprint}', '{{"status":"active"}}')"#
    ))
    .await
    .expect("live entity should be accepted");
    db.execute_unprepared(
        "INSERT INTO index_links (tenant_id, source_module, source_entity, source_schema_version, source_entity_id, source_locale_key, source_version, link_name, ordinal, target_module, target_entity, target_schema_version, target_entity_id, target_locale_key) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog', 'product', 1, '22222222-2222-2222-2222-222222222222', 'en-US', 1, 'channel', 0, 'channel', 'sales_channel', 1, '33333333-3333-3333-3333-333333333333', '')",
    )
    .await
    .expect("link should be bound to the exact source entity version");

    assert!(
        db.execute_unprepared(
            "UPDATE index_entities SET source_version = 2 WHERE tenant_id = '11111111-1111-1111-1111-111111111111' AND module_name = 'catalog' AND entity_name = 'product' AND schema_version = 1 AND entity_id = '22222222-2222-2222-2222-222222222222' AND locale_key = 'en-US'",
        )
        .await
        .is_err(),
        "source-version changes must not strand links on an older entity version"
    );
    assert!(
        db.execute_unprepared(
            "INSERT INTO index_links (tenant_id, source_module, source_entity, source_schema_version, source_entity_id, source_locale_key, source_version, link_name, ordinal, target_module, target_entity, target_schema_version, target_entity_id, target_locale_key) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog', 'product', 1, '22222222-2222-2222-2222-222222222222', 'en-US', 1, 'channel', 0, 'channel', 'sales_channel', 1, '44444444-4444-4444-4444-444444444444', '')",
        )
        .await
        .is_err(),
        "one link ordinal must identify exactly one ordered target"
    );
    assert!(
        db.execute_unprepared(&format!(
            "INSERT INTO index_entities (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key, source_version, schema_fingerprint, payload, is_deleted) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog', 'product', 1, '55555555-5555-5555-5555-555555555555', 'en-US', 1, '{fingerprint}', NULL, FALSE)"
        ))
        .await
        .is_err(),
        "live rows must retain a JSONB payload"
    );
    assert!(
        db.execute_unprepared(&format!(
            "INSERT INTO index_entities (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key, source_version, schema_fingerprint, payload, is_deleted) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog', 'product', 1, '66666666-6666-6666-6666-666666666666', 'en-US', 1, '{fingerprint}', '{{}}', TRUE)"
        ))
        .await
        .is_err(),
        "tombstones must not retain a stale payload"
    );

    db.execute_unprepared(
        "INSERT INTO index_inbox (tenant_id, source_name, delivery_id, mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, source_version, payload_hash) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog-source', 'delivery-1', 'upsert', 'catalog', 'product', 1, '22222222-2222-2222-2222-222222222222', 'en-US', 1, 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
    )
    .await
    .expect("pending inbox delivery should be accepted");
    assert!(
        db.execute_unprepared(
            "INSERT INTO index_inbox (tenant_id, source_name, delivery_id, mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, source_version, payload_hash, state) VALUES ('11111111-1111-1111-1111-111111111111', 'catalog-source', 'delivery-2', 'upsert', 'catalog', 'product', 1, '22222222-2222-2222-2222-222222222222', 'en-US', 2, 'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', 'processing')",
        )
        .await
        .is_err(),
        "processing deliveries require a complete lease"
    );
    assert!(
        db.execute_unprepared(
            "INSERT INTO index_checkpoints (tenant_id, checkpoint_kind, source_name, module_name, entity_name, schema_version, locale_key, partition_key, cursor, source_version) VALUES ('11111111-1111-1111-1111-111111111111', 'ingestion', 'catalog-source', 'catalog', 'product', 1, 'en-US', '', '{}', -1)",
        )
        .await
        .is_err(),
        "checkpoint source versions must be non-negative"
    );
    db.execute_unprepared(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, scope_kind, request) VALUES ('11111111-1111-1111-1111-111111111111', '77777777-7777-7777-7777-777777777777', 'rebuild', 'global', '{}')",
    )
    .await
    .expect("pending global job should be accepted");
    assert!(
        db.execute_unprepared(
            "INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, request) VALUES ('11111111-1111-1111-1111-111111111111', '88888888-8888-8888-8888-888888888888', 'rebuild', 'running', 'global', '{}')",
        )
        .await
        .is_err(),
        "running jobs require a complete lease"
    );
    db.execute_unprepared(
        "INSERT INTO index_reconciliation_recovery_audits (tenant_id, audit_id, job_id, actor_id, action, reason, prior_attempt_count, retry_epoch) VALUES ('11111111-1111-1111-1111-111111111111', '12121212-1212-1212-1212-121212121212', '77777777-7777-7777-7777-777777777777', '13131313-1313-1313-1313-131313131313', 'requeue', 'operator approved retry', 1, 1)",
    )
    .await
    .expect("bounded recovery audit should be accepted");
    assert!(
        db.execute_unprepared("UPDATE index_reconciliation_recovery_audits SET reason = 'changed'")
            .await
            .is_err(),
        "recovery audit rows must reject updates"
    );
    assert!(
        db.execute_unprepared("DELETE FROM index_reconciliation_recovery_audits")
            .await
            .is_err(),
        "recovery audit rows must reject deletes"
    );
    db.execute_unprepared(
        "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, scope_kind, details) VALUES ('11111111-1111-1111-1111-111111111111', '99999999-9999-9999-9999-999999999999', 'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd', 'entity_digest', 'error', 'global', '{}')",
    )
    .await
    .expect("open consistency finding should be accepted");
    assert!(
        db.execute_unprepared(
            "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, details) VALUES ('11111111-1111-1111-1111-111111111111', 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', 'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee', 'entity_digest', 'error', 'resolved', 'global', '{}')",
        )
        .await
        .is_err(),
        "closed findings require a closure timestamp"
    );

    for migration in migrations.iter().rev() {
        migration
            .down(&manager)
            .await
            .unwrap_or_else(|error| panic!("{} should roll back: {error}", migration.name()));
    }
    let remaining = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'index_%'"
                .to_owned(),
        ))
        .await
        .expect("remaining index tables should be queryable");
    assert!(
        remaining.is_empty(),
        "down migrations must remove all Index tables"
    );
}
