use rustok_core::module::MigrationSource;
use rustok_rbac::RbacModule;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn legacy_database() -> (DatabaseConnection, Box<dyn MigrationTrait>) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect SQLite");
    db.execute_unprepared(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
        CREATE TABLE roles (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
        CREATE TABLE permissions (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
        CREATE TABLE user_roles (user_id TEXT NOT NULL, role_id TEXT NOT NULL);
        CREATE TABLE role_permissions (role_id TEXT NOT NULL, permission_id TEXT NOT NULL);
        "#,
    )
    .await
    .expect("create RBAC parent tables");
    let manager = SchemaManager::new(&db);
    let mut migrations = RbacModule.migrations();
    assert_eq!(migrations.len(), 5);
    let cutover = migrations.pop().expect("append-only cutover migration");
    for migration in migrations {
        migration
            .up(&manager)
            .await
            .expect("apply historical RBAC migration");
    }
    (db, cutover)
}

async fn query_string(db: &DatabaseConnection, sql: &str, column: &str) -> String {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
        .await
        .expect("query")
        .expect("row")
        .try_get("", column)
        .expect("decode string")
}

async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!("SELECT COUNT(*) AS count FROM {table}"),
    ))
    .await
    .expect("count query")
    .expect("count row")
    .try_get("", "count")
    .expect("decode count")
}

async fn table_exists(db: &DatabaseConnection, table: &str) -> bool {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table.into()],
    ))
    .await
    .expect("table existence query")
    .expect("table existence row")
    .try_get::<i64>("", "count")
    .expect("decode table count")
        == 1
}

async fn add_tenant_scope_collision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    installation_id: Uuid,
) {
    let definition_id = Uuid::new_v4();
    let translation_id = Uuid::new_v4();
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .expect("disable foreign keys for corruption fixture");
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO rbac_artifact_permission_definitions
            (id, scope_key, installation_id, module_slug, release_digest, permission_key)
        VALUES
            ('{definition_id}', 'tenant:{tenant_id}', '{installation_id}', 'sample', 'sha256:tenant', 'sample.events.handle');
        INSERT INTO rbac_artifact_permission_translations
            (id, artifact_permission_id, locale, label, description)
        VALUES
            ('{translation_id}', '{definition_id}', 'en', 'Tenant handle', 'Tenant permission');
        "#
    ))
    .await
    .expect("add colliding tenant definition");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("restore foreign keys after corruption fixture");
}

#[tokio::test]
async fn legacy_catalog_grant_and_receipt_upgrade_and_rollback_truthfully() {
    let (db, migration) = legacy_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let catalog_id = Uuid::new_v4();
    let grant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let scope_key = format!("tenant:{tenant_id}");

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO roles (id, tenant_id) VALUES ('{role_id}', '{tenant_id}');
        INSERT INTO users (id, tenant_id) VALUES ('{actor_id}', '{tenant_id}');
        INSERT INTO rbac_artifact_permission_catalog
            (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description)
        VALUES
            ('{catalog_id}', '{scope_key}', '{installation_id}', 'sample', 'sha256:legacy', 'sample.events.handle', 'EN_us', 'Handle events', 'Allows event handling');
        INSERT INTO rbac_artifact_role_permissions
            (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id)
        VALUES
            ('{grant_id}', '{tenant_id}', '{role_id}', '{installation_id}', 'sample.events.handle', '{actor_id}');
        INSERT INTO rbac_artifact_role_permission_operations
            (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted)
        VALUES
            ('{operation_id}', '{tenant_id}', 'legacy-grant', '{role_id}', '{installation_id}', 'sample.events.handle', '{actor_id}', 1);
        "#
    ))
    .await
    .expect("seed legacy artifact authorization state");

    migration
        .up(&SchemaManager::new(&db))
        .await
        .expect("upgrade legacy artifact authorization state");

    assert_eq!(
        count(&db, "rbac_artifact_permission_installations").await,
        1
    );
    assert_eq!(count(&db, "rbac_artifact_permission_definitions").await, 1);
    assert_eq!(count(&db, "rbac_artifact_permission_translations").await, 1);
    assert_eq!(
        query_string(
            &db,
            "SELECT locale FROM rbac_artifact_permission_translations",
            "locale",
        )
        .await,
        "en-US"
    );
    assert_eq!(
        query_string(
            &db,
            "SELECT artifact_permission_id FROM rbac_artifact_role_permissions",
            "artifact_permission_id",
        )
        .await,
        catalog_id.to_string()
    );
    assert_eq!(
        query_string(
            &db,
            "SELECT permission_scope_key FROM rbac_artifact_role_permission_operations",
            "permission_scope_key",
        )
        .await,
        scope_key
    );

    migration
        .down(&SchemaManager::new(&db))
        .await
        .expect("roll back append-only cutover");
    assert_eq!(count(&db, "rbac_artifact_permission_catalog").await, 1);
    assert_eq!(
        query_string(
            &db,
            "SELECT locale FROM rbac_artifact_permission_catalog",
            "locale",
        )
        .await,
        "en-US"
    );
    assert_eq!(
        query_string(
            &db,
            "SELECT installation_id FROM rbac_artifact_role_permissions",
            "installation_id",
        )
        .await,
        installation_id.to_string()
    );
}

#[tokio::test]
async fn legacy_installation_with_platform_and_tenant_scope_fails_closed_atomically() {
    let (db, migration) = legacy_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let scope_key = format!("tenant:{tenant_id}");

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO roles (id, tenant_id) VALUES ('{role_id}', '{tenant_id}');
        INSERT INTO users (id, tenant_id) VALUES ('{actor_id}', '{tenant_id}');
        INSERT INTO rbac_artifact_permission_catalog
            (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description)
        VALUES
            ('{}', 'platform', '{installation_id}', 'sample', 'sha256:platform', 'sample.events.handle', 'en', 'Platform handle', 'Platform permission'),
            ('{}', '{scope_key}', '{installation_id}', 'sample', 'sha256:tenant', 'sample.events.handle', 'en', 'Tenant handle', 'Tenant permission');
        INSERT INTO rbac_artifact_role_permissions
            (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id)
        VALUES
            ('{}', '{tenant_id}', '{role_id}', '{installation_id}', 'sample.events.handle', '{actor_id}');
        "#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("seed ambiguous legacy selector");

    let error = migration
        .up(&SchemaManager::new(&db))
        .await
        .expect_err("ambiguous legacy selector must fail closed");
    assert!(
        error
            .to_string()
            .contains("installation identity is bound to conflicting scope")
    );
    assert!(table_exists(&db, "rbac_artifact_permission_catalog").await);
    assert!(!table_exists(&db, "rbac_artifact_permission_definitions").await);
    assert!(!table_exists(&db, "rbac_artifact_permission_definitions_new").await);
    assert!(!table_exists(&db, "rbac_artifact_permission_installations").await);
    assert_eq!(count(&db, "rbac_artifact_permission_catalog").await, 2);
    assert_eq!(count(&db, "rbac_artifact_role_permissions").await, 1);
}

#[tokio::test]
async fn canonical_grant_with_later_scope_collision_fails_rollback() {
    let (db, migration) = legacy_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let definition_id = Uuid::new_v4();

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO roles (id, tenant_id) VALUES ('{role_id}', '{tenant_id}');
        INSERT INTO users (id, tenant_id) VALUES ('{actor_id}', '{tenant_id}');
        INSERT INTO rbac_artifact_permission_catalog
            (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description)
        VALUES
            ('{definition_id}', 'platform', '{installation_id}', 'sample', 'sha256:platform', 'sample.events.handle', 'en', 'Platform handle', 'Platform permission');
        INSERT INTO rbac_artifact_role_permissions
            (id, tenant_id, role_id, installation_id, permission_key, granted_by_actor_id)
        VALUES
            ('{}', '{tenant_id}', '{role_id}', '{installation_id}', 'sample.events.handle', '{actor_id}');
        "#,
        Uuid::new_v4(),
    ))
    .await
    .expect("seed legacy platform grant");
    migration
        .up(&SchemaManager::new(&db))
        .await
        .expect("upgrade platform grant");
    add_tenant_scope_collision(&db, tenant_id, installation_id).await;

    let error = migration
        .down(&SchemaManager::new(&db))
        .await
        .expect_err("ambiguous canonical grant must block rollback");
    assert!(
        error
            .to_string()
            .contains("grant with ambiguous legacy selector")
    );
}

#[tokio::test]
async fn canonical_receipt_with_later_scope_collision_fails_rollback() {
    let (db, migration) = legacy_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let definition_id = Uuid::new_v4();

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO roles (id, tenant_id) VALUES ('{role_id}', '{tenant_id}');
        INSERT INTO users (id, tenant_id) VALUES ('{actor_id}', '{tenant_id}');
        INSERT INTO rbac_artifact_permission_catalog
            (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description)
        VALUES
            ('{definition_id}', 'platform', '{installation_id}', 'sample', 'sha256:platform', 'sample.events.handle', 'en', 'Platform handle', 'Platform permission');
        INSERT INTO rbac_artifact_role_permission_operations
            (id, tenant_id, idempotency_key, role_id, installation_id, permission_key, actor_id, granted)
        VALUES
            ('{}', '{tenant_id}', 'platform-receipt', '{role_id}', '{installation_id}', 'sample.events.handle', '{actor_id}', 1);
        "#,
        Uuid::new_v4(),
    ))
    .await
    .expect("seed legacy platform receipt");
    migration
        .up(&SchemaManager::new(&db))
        .await
        .expect("upgrade platform receipt");
    add_tenant_scope_collision(&db, tenant_id, installation_id).await;

    let error = migration
        .down(&SchemaManager::new(&db))
        .await
        .expect_err("ambiguous canonical receipt must block rollback");
    assert!(
        error
            .to_string()
            .contains("operation receipt with ambiguous legacy selector")
    );
}

#[tokio::test]
async fn late_sqlite_down_failure_restores_canonical_schema() {
    let (db, migration) = legacy_database().await;
    let tenant_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let definition_id = Uuid::new_v4();

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO roles (id, tenant_id) VALUES ('{role_id}', '{tenant_id}');
        INSERT INTO users (id, tenant_id) VALUES ('{actor_id}', '{tenant_id}');
        INSERT INTO rbac_artifact_permission_catalog
            (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description)
        VALUES
            ('{definition_id}', 'platform', '{installation_id}', 'sample', 'sha256:platform', 'sample.events.handle', 'en', 'Platform handle', 'Platform permission');
        "#
    ))
    .await
    .expect("seed legacy definition");
    migration
        .up(&SchemaManager::new(&db))
        .await
        .expect("upgrade definition");

    db.execute_unprepared(
        r#"
        CREATE TABLE conflicting_index_owner (id INTEGER PRIMARY KEY);
        CREATE INDEX rbac_artifact_permission_catalog_lookup_idx
            ON conflicting_index_owner (id);
        "#,
    )
    .await
    .expect("reserve legacy index name to force a late rollback failure");

    let error = migration
        .down(&SchemaManager::new(&db))
        .await
        .expect_err("late rollback DDL failure must be returned");
    assert!(error.to_string().contains("already exists"));
    assert!(table_exists(&db, "rbac_artifact_permission_definitions").await);
    assert!(table_exists(&db, "rbac_artifact_permission_translations").await);
    assert!(!table_exists(&db, "rbac_artifact_permission_catalog").await);
    assert!(!table_exists(&db, "rbac_artifact_permission_catalog_restore").await);
    assert_eq!(
        count(&db, "rbac_artifact_permission_installations").await,
        1
    );
    assert_eq!(count(&db, "rbac_artifact_permission_definitions").await, 1);
}
